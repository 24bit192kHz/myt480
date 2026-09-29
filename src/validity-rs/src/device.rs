//! A live, authenticated session with the sensor.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use log::{info, warn};

use crate::config::Config;
use crate::sensor::CaptureState;
use crate::tls::{HwKey, PairingMismatch, Tls};
use crate::usb::{Usb, Wedged};

pub struct Device {
    pub usb: Usb,
    pub tls: Tls,
    pub cfg: Config,
    pub capture: CaptureState,
    pub hwkey: HwKey,
    pub cancel: Arc<AtomicBool>,
    /// When the last capture was aborted (see `Device::capture`).
    pub last_abort: Option<Instant>,
}

/// The sensor was told to reboot; the handle is dead and must be reopened.
#[derive(Debug)]
pub struct Rebooted;

impl std::fmt::Display for Rebooted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("sensor rebooted")
    }
}

impl std::error::Error for Rebooted {}

impl Device {
    /// Open the USB device without talking TLS yet.
    pub fn connect(cfg: &Config, cancel: Arc<AtomicBool>) -> Result<Device> {
        let usb = Usb::open(cancel.clone())?;
        let hwkey = cfg.hwkeys.first().cloned().unwrap_or_else(HwKey::from_dmi);
        Ok(Device { usb, tls: Tls::new(&hwkey), cfg: cfg.clone(), capture: CaptureState::default(), hwkey, cancel, last_abort: None })
    }

    pub fn cmd(&mut self, cmd: &[u8]) -> Result<Vec<u8>> {
        self.tls.cmd(&self.usb, cmd)
    }

    pub fn app(&mut self, cmd: &[u8]) -> Result<Vec<u8>> {
        self.tls.app(&self.usb, cmd)
    }

    /// Unlock the pairing blob with the first host identity that fits.
    fn load_pairing(&mut self) -> Result<()> {
        let flash = self.read_tls_flash()?;
        for hw in self.cfg.hwkeys.clone() {
            let mut tls = Tls::new(&hw);
            match tls.parse_tls_flash(&flash) {
                Ok(()) => {
                    info!("pairing unlocked with host identity {:?} / {:?}", hw.product_name, hw.serial);
                    self.tls = tls;
                    self.hwkey = hw;
                    return Ok(());
                }
                Err(e) if e.downcast_ref::<PairingMismatch>().is_some() => {
                    info!("host identity {:?} / {:?} does not fit the pairing", hw.product_name, hw.serial);
                }
                Err(e) => return Err(e),
            }
        }
        Err(PairingMismatch.into())
    }

    /// Full bring-up: hello, pairing (first run), TLS, firmware, calibration, DB.
    /// Returns `Rebooted` when the sensor had to restart (after pairing or a firmware upload).
    pub fn init(&mut self) -> Result<()> {
        self.tls.reset();
        self.usb.drain_int();
        self.usb.wait_ready()?;
        // python-validity's order: pair a blank sensor first, then the hello sequence.
        self.init_flash()?;
        self.usb.send_init()?;
        self.load_pairing()?;
        self.tls.open(&self.usb).context("TLS handshake")?;
        self.upload_fwext()?;
        self.open_sensor()?;
        self.init_db()?;
        Ok(())
    }

    /// Open and initialize, riding out sensor reboots.
    pub fn open(cfg: &Config, cancel: Arc<AtomicBool>) -> Result<Device> {
        let started = Instant::now();
        for attempt in 1..=4 {
            let mut dev = match Device::connect(cfg, cancel.clone()) {
                Ok(d) => d,
                Err(e) if attempt < 4 => {
                    warn!("open attempt {attempt}: {e:#}");
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                Err(e) => return Err(e),
            };
            match dev.init() {
                Ok(()) => {
                    info!("sensor ready in {} ms", started.elapsed().as_millis());
                    return Ok(dev);
                }
                Err(e) if e.downcast_ref::<Rebooted>().is_some() => {
                    info!("sensor rebooted during init; reconnecting");
                    dev.usb.reset();
                    drop(dev);
                    std::thread::sleep(Duration::from_millis(300));
                }
                Err(e) if e.downcast_ref::<Wedged>().is_some() && attempt < 4 => {
                    warn!("{e:#}; resetting the USB device");
                    dev.usb.reset();
                    if attempt >= 2 {
                        // A port reset was not enough: reboot the sensor itself.
                        let _ = dev.usb.cmd_timeout(&[0x05, 0x02, 0x00], Duration::from_secs(2));
                        dev.usb.reset();
                        drop(dev);
                        std::thread::sleep(Duration::from_millis(300));
                    } else {
                        drop(dev);
                        std::thread::sleep(Duration::from_millis(300));
                    }
                }
                Err(e) => return Err(e),
            }
        }
        bail!("sensor kept rebooting during init")
    }

    /// Re-run init on the same handle (after resume); reconnects if that fails.
    pub fn reinit(self) -> Result<Device> {
        let (cfg, cancel) = (self.cfg.clone(), self.cancel.clone());
        let mut dev = self;
        match dev.init() {
            Ok(()) => Ok(dev),
            Err(e) => {
                warn!("re-init failed ({e:#}); reopening the device");
                drop(dev);
                Device::open(&cfg, cancel)
            }
        }
    }

    /// Reboot the sensor and release the device. Without the reboot the sensor
    /// accumulates TLS sessions and eventually runs out of memory.
    pub fn close(mut self) {
        // The reboot is internal to the sensor (it stays on the bus); the next
        // open polls the hello until the firmware is back.
        if self.tls.is_secure() {
            let _ = self.reboot();
        }
        self.usb.reset();
    }
}

