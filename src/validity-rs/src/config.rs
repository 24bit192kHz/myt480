//! /etc/validity-rs.conf — `key = value` lines, `#` comments.
//!
//! ```text
//! # Host identity the pairing is sealed with, tried before the live DMI values.
//! hwkey = T480 123456789
//! # Extra identities to try (e.g. the stock BIOS's) if the first ones fail.
//! hwkey_fallback = 20L6S4VC00 PF0XXXXX
//! # Map a user to a Windows SID (default S-1-5-21-111111111-1111111111-1111111111-<uid>).
//! sid.alice = S-1-5-21-1-2-3-1001
//! data_dir = /var/lib/validity-rs
//! retry_delay_ms = 1000
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::tls::HwKey;

pub const DEFAULT_PATH: &str = "/etc/validity-rs.conf";

#[derive(Clone, Debug)]
pub struct Config {
    pub hwkeys: Vec<HwKey>,
    pub sids: HashMap<String, String>,
    pub data_dir: PathBuf,
    /// Extra directories searched for calib-data.bin / the fwext file (python-validity's).
    pub import_dirs: Vec<PathBuf>,
    pub retry_delay: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hwkeys: Vec::new(),
            sids: HashMap::new(),
            data_dir: PathBuf::from("/var/lib/validity-rs"),
            import_dirs: vec![PathBuf::from("/var/lib/python-validity"), PathBuf::from("/var/run/python-validity")],
            retry_delay: Duration::from_millis(1000),
        }
    }
}

fn parse_hwkey(v: &str) -> Result<HwKey> {
    // "name serial"; the serial is the last word so names may contain spaces.
    let v = v.trim();
    let (name, serial) = v.rsplit_once(char::is_whitespace).context("hwkey needs a product name and a serial")?;
    Ok(HwKey { product_name: name.trim().into(), serial: serial.into() })
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        let mut cfg = Config::default();
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
        };
        let mut pinned = Vec::new();
        let mut fallback = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                bail!("{}:{}: expected key = value", path.display(), n + 1);
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "hwkey" => pinned.push(parse_hwkey(v)?),
                "hwkey_fallback" => fallback.push(parse_hwkey(v)?),
                "data_dir" => cfg.data_dir = v.into(),
                "retry_delay_ms" => cfg.retry_delay = Duration::from_millis(v.parse().context("retry_delay_ms")?),
                _ if k.starts_with("sid.") => {
                    cfg.sids.insert(k[4..].to_string(), v.to_string());
                }
                _ => bail!("{}:{}: unknown key {k}", path.display(), n + 1),
            }
        }
        // Order: pinned identities, the live DMI identity, then fallbacks.
        cfg.hwkeys = pinned;
        let dmi = HwKey::from_dmi();
        if !cfg.hwkeys.contains(&dmi) {
            cfg.hwkeys.push(dmi);
        }
        for k in fallback {
            if !cfg.hwkeys.contains(&k) {
                cfg.hwkeys.push(k);
            }
        }
        Ok(cfg)
    }

    pub fn sid_for(&self, user: &str) -> Result<crate::db::Sid> {
        if user.starts_with("S-") {
            return crate::db::Sid::parse(user);
        }
        if let Some(s) = self.sids.get(user) {
            return crate::db::Sid::parse(s);
        }
        let uid = uid_of(user).with_context(|| format!("unknown user {user}"))?;
        Ok(crate::db::Sid::for_uid(uid))
    }
}

/// Look a user up in /etc/passwd (no NSS; the T480 only has local users).
pub fn uid_of(user: &str) -> Option<u32> {
    let pw = std::fs::read_to_string("/etc/passwd").ok()?;
    pw.lines().find_map(|l| {
        let mut f = l.split(':');
        (f.next()? == user).then(|| f.nth(1)?.parse().ok())?
    })
}

/// Reverse of the default SID mapping, for display.
pub fn uid_of_sid(sid: &crate::db::Sid) -> Option<String> {
    let d = crate::db::Sid::for_uid(0);
    if sid.revision != d.revision || sid.auth != d.auth || sid.subauth.len() != 5 || sid.subauth[..4] != d.subauth[..4] {
        return None;
    }
    let uid = sid.subauth[4];
    let pw = std::fs::read_to_string("/etc/passwd").ok()?;
    pw.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() > 2 && f[2].parse() == Ok(uid)).then(|| f[0].to_string())
    })
}
