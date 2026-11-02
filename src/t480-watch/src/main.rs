//! t480-watch: the session's polling loops without a fork per tick.
//!
//! The shell versions started 5-6 programs every 2 s each (pgrep, cat, iw, awk,
//! sleep), around the clock and above all while the screen sat locked; this does
//! the same checks with plain file reads and starts a program only when
//! something changes.
//!
//!   t480-watch lock-powersave        (also when started as `lock-powersave`)
//!       While slock runs: panel off after LOCK_POWERSAVE_SECS (120) s; Wi-Fi
//!       power save on while on battery, off on AC. At unlock: panel on and
//!       DPMS timeouts off again, Wi-Fi power save off. LOCK_POWERSAVE_WLAN
//!       (wlan0) and LOCK_POWERSAVE_LOCKER (slock) override the defaults.
//!   t480-watch monitors <monitor.sh>
//!       Runs `monitor.sh` (no arguments = apply once) now and whenever the
//!       kernel's connector states change and hold for 2 s. Singleton through
//!       $XDG_RUNTIME_DIR/monitor-watch.lock, as `monitor.sh watch` was.

use std::fs::{self, File};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_secs(2);

fn read_trim(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim_end().to_owned())
}

fn run(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

// ---------------------------------------------------------------- lock-powersave

/// A live (not zombie) process whose comm is exactly `name`?
/// /proc/PID/stat is "PID (comm) STATE ..."; comm may itself hold ')'.
fn is_live(pid: u32, name: &str) -> bool {
    let Some(stat) = read_trim(format!("/proc/{pid}/stat")) else { return false };
    let Some((head, tail)) = stat.rsplit_once(')') else { return false };
    head.split_once('(').is_some_and(|(_, comm)| comm == name)
        && tail.split_whitespace().next().is_some_and(|st| st != "Z")
}

/// pgrep -x, minus zombies (an exited locker its parent has not reaped yet).
fn find_proc(name: &str) -> Option<u32> {
    fs::read_dir("/proc").ok()?.flatten().find_map(|e| {
        let pid: u32 = e.file_name().to_str()?.parse().ok()?;
        is_live(pid, name).then_some(pid)
    })
}

/// Still the same locker? (a pid can be reused, so compare comm too)
fn alive(pid: u32, name: &str) -> bool {
    is_live(pid, name)
}

fn on_battery() -> bool {
    read_trim("/sys/class/power_supply/AC/online").as_deref() == Some("0")
}

/// `iw dev W get power_save` -> Some(true) for "on"
fn wifi_ps_get(wlan: &str) -> Option<bool> {
    let out = Command::new("iw")
        .args(["dev", wlan, "get", "power_save"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    match String::from_utf8_lossy(&out.stdout).split_whitespace().last()? {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

struct WifiPs {
    wlan: String,
    known: Option<bool>,
    checked: Option<Instant>,
}

impl WifiPs {
    /// Set power save to `want` only when it differs. The current state is read
    /// again when the wish changes and every 30 s otherwise (a resume or the
    /// driver can reset it), not on every tick.
    fn ensure(&mut self, want: bool) {
        let stale = self.checked.is_none_or(|t| t.elapsed() >= Duration::from_secs(30));
        if self.known != Some(want) || stale {
            self.known = wifi_ps_get(&self.wlan);
            self.checked = Some(Instant::now());
        }
        if let Some(cur) = self.known
            && cur != want
        {
            let v = if want { "on" } else { "off" };
            // sudoers.d/lock-powersave allows exactly this command
            if run("sudo", &["-n", "/usr/bin/iw", "dev", &self.wlan, "set", "power_save", v]) {
                self.known = Some(want);
            }
        }
    }
}

fn lock_powersave() -> ExitCode {
    if std::env::var_os("DISPLAY").is_none() {
        // SAFETY: single-threaded, before any child is started
        unsafe { std::env::set_var("DISPLAY", ":0") };
    }
    let lock_after = Duration::from_secs(
        std::env::var("LOCK_POWERSAVE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(120),
    );
    let mut ps = WifiPs {
        wlan: std::env::var("LOCK_POWERSAVE_WLAN").unwrap_or_else(|_| "wlan0".into()),
        known: None,
        checked: None,
    };
    let locker = std::env::var("LOCK_POWERSAVE_LOCKER").unwrap_or_else(|_| "slock".into());
    let locker = locker.as_str();
    let mut slock: Option<u32> = None;
    let mut since: Option<Instant> = None; // locked since
    let mut off = false;
    loop {
        sleep(TICK);
        // a known locker costs one read; only an unlocked screen needs the /proc scan
        if !slock.is_some_and(|p| alive(p, locker)) {
            slock = find_proc(locker);
        }
        if slock.is_some() {
            let t0 = *since.get_or_insert_with(Instant::now);
            // +dpms: gxwc idles only when DPMS is enabled
            if !off && t0.elapsed() + TICK > lock_after {
                off = run("xset", &["+dpms", "dpms", "force", "off"]);
            }
            ps.ensure(on_battery());
        } else {
            if off {
                // back to no DPMS timeouts for the unlocked desktop
                run("xset", &["dpms", "force", "on", "-dpms"]);
                off = false;
            }
            since = None;
            ps.ensure(false);
        }
    }
}

// ---------------------------------------------------------------- monitors

/// Connector states as the kernel last saw them (reading does not probe):
/// the same string `state()` in monitor.sh built.
fn connector_state() -> String {
    let mut v: Vec<(String, String)> = fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().into_string().ok()?;
            if !(n.starts_with("card") && n.contains('-')) {
                return None;
            }
            let s = read_trim(e.path().join("status"))?;
            Some((n, s))
        })
        .collect();
    v.sort();
    v.into_iter().map(|(n, s)| format!("{n}={s} ")).collect()
}

fn monitors(script: &str) -> ExitCode {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let Ok(lock) = File::options().create(true).append(true).open(format!("{dir}/monitor-watch.lock"))
    else {
        eprintln!("t480-watch: cannot open the lock file");
        return ExitCode::FAILURE;
    };
    if lock.try_lock().is_err() {
        return ExitCode::SUCCESS; // another watcher runs
    }
    let apply = || {
        // the lock must not leak into monitor.sh: it is close-on-exec (std opens with O_CLOEXEC)
        run(script, &[]);
    };
    let mut last = connector_state();
    apply();
    loop {
        sleep(TICK);
        let now = connector_state();
        if now == last {
            continue;
        }
        // HDMI presence comes from edid-pind, so a new state only has to hold for 2 s
        sleep(TICK);
        if connector_state() != now {
            continue;
        }
        last = now;
        apply();
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let argv0 = args.first().map(|a| a.rsplit('/').next().unwrap_or(a)).unwrap_or("");
    let (cmd, rest) = if argv0 == "lock-powersave" {
        ("lock-powersave", &args[1..])
    } else {
        (args.get(1).map(String::as_str).unwrap_or(""), args.get(2..).unwrap_or(&[]))
    };
    match (cmd, rest) {
        ("lock-powersave", []) => lock_powersave(),
        ("monitors", [script]) => monitors(script),
        _ => {
            eprintln!("usage: t480-watch lock-powersave | t480-watch monitors <monitor.sh>");
            ExitCode::from(2)
        }
    }
}
