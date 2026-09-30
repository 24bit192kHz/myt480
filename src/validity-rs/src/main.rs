//! validity-rs: a Rust port of python-validity for the ThinkPad T480's
//! Synaptics 06cb:009a match-on-chip fingerprint sensor.

mod blobs;
mod config;
mod db;
mod dbus;
mod device;
mod flash;
mod pair;
mod sensor;
mod timeslot;
mod tls;
mod usb;
mod util;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tls_emu_tests;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use log::{LevelFilter, error, info};

use crate::config::Config;
use crate::device::Device;
use crate::sensor::Progress;

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::max_level()
    }

    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            if log::max_level() >= LevelFilter::Debug {
                let t = START.get_or_init(Instant::now).elapsed();
                eprintln!("{:>6}.{:03} {:<5} {}", t.as_secs(), t.subsec_millis(), r.level(), r.args());
            } else {
                eprintln!("{:<5} {}", r.level(), r.args());
            }
        }
    }

    fn flush(&self) {}
}

const USAGE: &str = "usage: validity-rs [-v|-vv] [-c CONFIG] COMMAND

commands:
  daemon                 serve io.github.uunicorn.Fprint.Device for open-fprintd (default)
  info                   show sensor, firmware, pairing and enrolled fingers
  identify               wait for a finger and match it on the chip
  list USER              list USER's enrolled fingers
  enroll USER FINGER     enroll a finger (e.g. right-index-finger)
  delete USER            delete all of USER's fingers
  calibrate              recalibrate and store a new clean-slate image
  calib-check            measure how well the stored calibration fits (writes nothing)
  led                    flash the sensor LED
  raw HEX                send a raw command over TLS and print the reply
  factory-reset --yes    wipe pairing, firmware and fingers on the sensor

The standalone commands need the sensor to themselves: stop the daemon first.";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut level = LevelFilter::Info;
    let mut cfg_path = PathBuf::from(config::DEFAULT_PATH);
    while let Some(a) = args.first().cloned() {
        match a.as_str() {
            "-v" => level = LevelFilter::Debug,
            "-vv" => level = LevelFilter::Trace,
            "-c" if args.len() > 1 => {
                cfg_path = args[1].clone().into();
                args.remove(0);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            _ => break,
        }
        args.remove(0);
    }
    let _ = log::set_logger(&Logger);
    log::set_max_level(level);

    if let Err(e) = run(&cfg_path, &args) {
        error!("{e:#}");
        std::process::exit(1);
    }
}

fn block_signals() -> libc::sigset_t {
    // Block the termination signals before any thread starts so sigwait sees them.
    let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGHUP);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
    set
}

fn wait_signal(set: &libc::sigset_t) -> i32 {
    let mut sig = 0;
    unsafe { libc::sigwait(set, &mut sig) };
    sig
}

/// CLI commands: the first signal cancels the capture (so the sensor is closed
/// cleanly), a second one exits immediately.
fn open(cfg: &Config) -> Result<Device> {
    let set = block_signals();
    let cancel = Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    std::thread::spawn(move || {
        wait_signal(&set);
        eprintln!("cancelling…");
        c.store(true, std::sync::atomic::Ordering::SeqCst);
        wait_signal(&set);
        std::process::exit(130);
    });
    Device::open(cfg, cancel)
}

fn run(cfg_path: &std::path::Path, args: &[String]) -> Result<()> {
    let cfg = Config::load(cfg_path)?;
    let cmd = args.first().map(String::as_str).unwrap_or("daemon");
    let arg = |i: usize| args.get(i).map(String::as_str).with_context(|| format!("missing argument\n\n{USAGE}"));
    match cmd {
        "daemon" => daemon(cfg),
        "info" => {
            let mut d = open(&cfg)?;
            let r = d.info();
            d.close();
            r
        }
        "identify" => {
            let mut d = open(&cfg)?;
            println!("touch the sensor");
            let t = Instant::now();
            let r = d.identify(&mut |p| {
                if let Progress::RetryScan(e) = p {
                    println!("retry: {e}");
                }
            });
            let users = d.users();
            d.close();
            match r? {
                Some((uid, sub, _)) => {
                    let who = users?.into_iter().find(|u| u.dbid as u32 == uid).map(|u| u.identity.to_string());
                    println!("match: user {} ({}) finger {} in {} ms", uid, who.unwrap_or_default(), blobs::finger_name(sub), t.elapsed().as_millis());
                }
                None => println!("no match ({} ms)", t.elapsed().as_millis()),
            }
            Ok(())
        }
        "list" => {
            let sid = cfg.sid_for(arg(1)?)?;
            let mut d = open(&cfg)?;
            let u = d.lookup_user(&sid);
            d.close();
            for f in u?.map(|u| u.fingers).unwrap_or_default() {
                println!("{}", blobs::finger_name(f.subtype));
            }
            Ok(())
        }
        "enroll" => {
            let sid = cfg.sid_for(arg(1)?)?;
            let finger = arg(2)?;
            let sub = blobs::finger_id(finger).with_context(|| format!("unknown finger name {finger}"))?;
            let mut d = open(&cfg)?;
            println!("touch the sensor repeatedly with your {finger}");
            let r = d.enroll(&sid, sub, &mut |p| match p {
                Progress::StagePassed => println!("stage passed"),
                Progress::RetryScan(e) => println!("retry: {e}"),
            });
            d.close();
            println!("enrolled as record {}", r?);
            Ok(())
        }
        "delete" => {
            let sid = cfg.sid_for(arg(1)?)?;
            let mut d = open(&cfg)?;
            let r = (|| {
                if let Some(u) = d.lookup_user(&sid)? {
                    d.del_record(u.dbid)?;
                    println!("deleted {} finger(s)", u.fingers.len());
                } else {
                    println!("no fingers enrolled");
                }
                Ok(())
            })();
            d.close();
            r
        }
        "calibrate" => {
            let mut d = open(&cfg)?;
            let r = d.run_calibration().and_then(|_| {
                std::fs::create_dir_all(&d.cfg.data_dir)?;
                std::fs::write(d.cfg.data_dir.join("calib-data.bin"), &d.capture.calib_data)?;
                Ok(())
            });
            d.close();
            r
        }
        "calib-check" => {
            let mut d = open(&cfg)?;
            let r = d.calib_check();
            d.close();
            r
        }
        "led" => {
            let mut d = open(&cfg)?;
            let r = d.led_test();
            d.close();
            r
        }
        "raw" => {
            let bytes = hex::decode(arg(1)?)?;
            let mut d = open(&cfg)?;
            let r = d.app(&bytes);
            d.close();
            println!("{}", hex::encode(r?));
            Ok(())
        }
        "factory-reset" => {
            if arg(1).ok() != Some("--yes") {
                bail!("factory-reset wipes the pairing and every enrolled finger; pass --yes");
            }
            let mut d = Device::connect(&cfg, Arc::new(AtomicBool::new(false)))?;
            d.usb.wait_ready()?;
            d.usb.send_init()?;
            let r = d.factory_reset();
            d.usb.reset();
            r?;
            println!("sensor reset; the next start pairs it again");
            Ok(())
        }
        other => bail!("unknown command {other}\n\n{USAGE}"),
    }
}

fn daemon(cfg: Config) -> Result<()> {
    let set = block_signals();

    let dev = match Device::open(&cfg, Arc::new(AtomicBool::new(false))) {
        Ok(d) => d,
        Err(e) => {
            // Don't let the supervisor hammer a sick sensor.
            std::thread::sleep(std::time::Duration::from_secs(5));
            return Err(e);
        }
    };
    let shared = dbus::Shared::new(dev, cfg);
    dbus::serve(shared.clone())?;
    info!("serving {} at {}", dbus::IFACE, dbus::PATH);

    let sig = wait_signal(&set);
    info!("caught signal {sig}; rebooting the sensor and exiting");
    shared.shutdown();
    Ok(())
}

impl Device {
    fn info(&mut self) -> Result<()> {
        let rom = self.rom_info()?;
        let (major, ver, name) = self.identify_sensor()?;
        println!("sensor      {name} (major {major:#x}, version {ver:#x})");
        println!("rom         {}.{} build {} product {:#x} u1 {} (timestamp {})", rom.major, rom.minor, rom.build, rom.product, rom.u1, rom.timestamp);
        if let Some(fw) = self.get_fw_info(2)? {
            println!("fwext       {}.{} ({} modules, built {})", fw.major, fw.minor, fw.modules, fw.buildtime);
        }
        println!("pairing     {:?} / {:?}", self.hwkey.product_name, self.hwkey.serial);
        let fi = self.get_flash_info()?;
        println!("flash       {:x}:{:x} {}x{}", fi.jid0, fi.jid1, fi.blocks, fi.blocksize);
        for p in &fi.partitions {
            println!("  part {} type {} access {} @{:#08x} size {:#x}", p.id, p.typ, p.access, p.offset, p.size);
        }
        let db = self.db_info()?;
        println!("template db {} used / {} free of {}, {} records", db.used, db.free, db.total, db.records);
        for u in self.users()? {
            let who = config::uid_of_sid(&u.identity);
            println!("  user {} {}{}", u.dbid, u.identity, who.map(|n| format!(" ({n})")).unwrap_or_default());
            for f in &u.fingers {
                println!("    finger {} {} ({} bytes)", f.dbid, blobs::finger_name(f.subtype), f.value_size);
            }
        }
        Ok(())
    }
}
