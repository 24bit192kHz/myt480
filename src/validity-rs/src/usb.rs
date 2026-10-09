//! Raw USB transport for the 06cb:009a sensor.
//!
//! EP 0x01 OUT / 0x81 IN carry commands, 0x82 IN bulk image/calibration data,
//! 0x83 IN interrupt events (finger down, capture progress, match result).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use log::{debug, trace};
use rusb::{DeviceHandle, GlobalContext};

use crate::blobs;
use crate::util::{check_status, status};

pub const VID: u16 = 0x06cb;
pub const PID: u16 = 0x009a;

const EP_OUT: u8 = 0x01;
const EP_CMD_IN: u8 = 0x81;
const EP_DATA_IN: u8 = 0x82;
const EP_INT_IN: u8 = 0x83;

const CMD_TIMEOUT: Duration = Duration::from_secs(15);
const DATA_TIMEOUT: Duration = Duration::from_secs(10);
/// Interrupt reads return as soon as an event arrives; this only bounds how
/// quickly a cancel request is noticed.
const INT_POLL: Duration = Duration::from_millis(50);
const HELLO_TIMEOUT: Duration = Duration::from_secs(2);

/// The sensor did not answer the first hello (left mid-operation by a dead process).
#[derive(Debug)]
pub struct Wedged;

impl std::fmt::Display for Wedged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("sensor does not answer (left mid-operation?)")
    }
}

impl std::error::Error for Wedged {}

#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("operation cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// A USB-level failure (device gone, pipe error). Callers treat it as fatal
/// for the current session and reopen the device.
#[derive(Debug)]
pub struct UsbFailure(pub rusb::Error);

impl std::fmt::Display for UsbFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "usb: {}", self.0)
    }
}

impl std::error::Error for UsbFailure {}

/// Anything that carries a command to the sensor and returns its reply.
/// `Usb` is the real one; tests substitute an emulated sensor.
pub trait Transport {
    fn cmd(&self, out: &[u8]) -> Result<Vec<u8>>;
}

impl Transport for Usb {
    fn cmd(&self, out: &[u8]) -> Result<Vec<u8>> {
        Usb::cmd(self, out)
    }
}

pub struct Usb {
    handle: DeviceHandle<GlobalContext>,
    cancel: Arc<AtomicBool>,
}

/// Keep the cancellable event policy independent of the USB handle so event
/// ordering and cancellation races can be tested without claiming a sensor.
fn wait_interrupt(
    cancel: &AtomicBool,
    mut read: impl FnMut(&mut [u8]) -> rusb::Result<usize>,
) -> Result<Vec<u8>> {
    let mut buf = [0u8; 1024];
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(Cancelled.into());
        }
        match read(&mut buf) {
            Ok(n) => {
                // A ready interrupt must not hide a cancel that arrived during
                // the read, including one racing the finger-down event.
                if cancel.load(Ordering::SeqCst) {
                    return Err(Cancelled.into());
                }
                trace!("<int< {}", hex::encode(&buf[..n]));
                return Ok(buf[..n].to_vec());
            }
            Err(rusb::Error::Timeout) => {}
            Err(e) => return Err(UsbFailure(e).into()),
        }
    }
}

impl Usb {
    pub fn open(cancel: Arc<AtomicBool>) -> Result<Self> {
        let handle = rusb::open_device_with_vid_pid(VID, PID)
            .with_context(|| format!("no {VID:04x}:{PID:04x} device found"))?;
        let dev = handle.device();
        debug!("opened usb {}-{} ({VID:04x}:{PID:04x})", dev.bus_number(), dev.address());
        // pyusb's set_configuration() + implicit claim.
        handle.set_active_configuration(1).map_err(UsbFailure)?;
        handle.claim_interface(0).map_err(UsbFailure)?;
        Ok(Usb { handle, cancel })
    }

    pub fn reset(&mut self) {
        let _ = self.handle.release_interface(0);
        let _ = self.handle.reset();
    }

    pub fn cmd(&self, out: &[u8]) -> Result<Vec<u8>> {
        self.cmd_timeout(out, CMD_TIMEOUT)
    }

    pub fn cmd_timeout(&self, out: &[u8], timeout: Duration) -> Result<Vec<u8>> {
        if out.is_empty() {
            bail!("empty command");
        }
        trace!(">cmd> {}", hex::encode(out));
        self.handle.write_bulk(EP_OUT, out, timeout).map_err(UsbFailure)?;
        let mut buf = vec![0u8; 100 * 1024];
        let n = self.handle.read_bulk(EP_CMD_IN, &mut buf, timeout).map_err(UsbFailure)?;
        buf.truncate(n);
        trace!("<cmd< {}", hex::encode(&buf));
        Ok(buf)
    }

    pub fn read_data(&self) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; 1024 * 1024];
        let n = self.handle.read_bulk(EP_DATA_IN, &mut buf, DATA_TIMEOUT).map_err(UsbFailure)?;
        buf.truncate(n);
        trace!("<data< {n} bytes");
        Ok(buf)
    }

    /// Wait for the next interrupt event; returns `Cancelled` once the shared
    /// cancel flag is raised. Unlike python-validity the flag is not cleared
    /// here. Check it before reading and before accepting a successful event so
    /// an uninterrupted event stream cannot starve cancellation.
    pub fn wait_int(&self) -> Result<Vec<u8>> {
        wait_interrupt(&self.cancel, |buf| {
            self.handle.read_interrupt(EP_INT_IN, buf, INT_POLL)
        })
    }

    /// Wait for an interrupt event ignoring cancel requests: used once the sensor
    /// is committed (finger down, match or enrollment in progress).
    pub fn wait_int_for(&self, timeout: Duration) -> Result<Vec<u8>> {
        let mut buf = [0u8; 1024];
        match self.handle.read_interrupt(EP_INT_IN, &mut buf, timeout) {
            Ok(n) => {
                trace!("<int< {}", hex::encode(&buf[..n]));
                Ok(buf[..n].to_vec())
            }
            // A missing event on a live link (finger lifted mid-capture) is not a USB fault.
            Err(rusb::Error::Timeout) => bail!("sensor sent no event within {} s", timeout.as_secs()),
            Err(e) => Err(UsbFailure(e).into()),
        }
    }

    /// Drain any stale interrupt events left over from an aborted operation.
    pub fn drain_int(&self) {
        let mut buf = [0u8; 1024];
        while let Ok(n) = self.handle.read_interrupt(EP_INT_IN, &mut buf, Duration::from_millis(5)) {
            trace!("<int< (drained) {}", hex::encode(&buf[..n]));
        }
    }

    /// Pre-TLS hello sequence; loads the "clean slate" program when no
    /// firmware extension is present.
    /// Probe the sensor with the ROM-info hello until it answers `0000`.
    pub fn wait_ready(&self) -> Result<()> {
        // A sensor left mid-operation by a killed process ignores commands;
        // fail fast so the caller can reset it.
        // Right after a reboot the sensor answers with a transient status (e.g. 582c, 0315)
        // for a moment; poll until it is ready. No answer at all means it is wedged.
        let t = std::time::Instant::now();
        loop {
            let hello = self.cmd_timeout(&[0x01], HELLO_TIMEOUT).map_err(|e| e.context(Wedged))?;
            match check_status(&hello) {
                Ok(()) => break,
                Err(e) if t.elapsed() < Duration::from_secs(5) => {
                    debug!("sensor not ready yet ({e}); retrying");
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(e.context(Wedged)),
            }
        }
        if t.elapsed() > Duration::from_millis(50) {
            debug!("sensor became ready after {} ms", t.elapsed().as_millis());
        }
        Ok(())
    }

    pub fn send_init(&self) -> Result<()> {
        check_status(&self.cmd(&[0x01])?)?;
        check_status(&self.cmd(&[0x19])?)?;
        let fw = self.cmd(&[0x43, 0x02])?;
        check_status(&self.cmd(blobs::INIT_HARDCODED)?)?;
        if status(&fw) != 0 {
            debug!("no firmware extension: sending clean-slate init");
            self.cmd(blobs::INIT_HARDCODED_CLEAN_SLATE)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_event_preserves_its_bytes() {
        let cancel = AtomicBool::new(false);
        let event = wait_interrupt(&cancel, |buf| {
            buf[..3].copy_from_slice(&[3, 0, 4]);
            Ok(3)
        })
        .unwrap();
        assert_eq!(event, [3, 0, 4]);
    }

    #[test]
    fn timeout_retries_until_an_event() {
        let cancel = AtomicBool::new(false);
        let mut reads = 0;
        let event = wait_interrupt(&cancel, |buf| {
            reads += 1;
            if reads == 1 {
                return Err(rusb::Error::Timeout);
            }
            buf[0] = 2;
            Ok(1)
        })
        .unwrap();
        assert_eq!(event, [2]);
        assert_eq!(reads, 2);
    }

    #[test]
    fn cancellation_before_wait_does_not_read() {
        let cancel = AtomicBool::new(true);
        let mut reads = 0;
        let result = wait_interrupt(&cancel, |buf| {
            reads += 1;
            buf[0] = 1;
            Ok(1)
        });
        assert!(result.unwrap_err().is::<Cancelled>());
        assert_eq!(reads, 0);
    }

    #[test]
    fn timeout_observes_cancellation_during_the_read() {
        let cancel = AtomicBool::new(false);
        let mut reads = 0;
        let result = wait_interrupt(&cancel, |_| {
            reads += 1;
            cancel.store(true, Ordering::SeqCst);
            Err(rusb::Error::Timeout)
        });
        assert!(result.unwrap_err().is::<Cancelled>());
        assert_eq!(reads, 1);
    }

    #[test]
    fn cancellation_during_read_wins_over_finger_down() {
        let cancel = AtomicBool::new(false);
        let result = wait_interrupt(&cancel, |buf| {
            cancel.store(true, Ordering::SeqCst);
            buf[0] = 2;
            Ok(1)
        });
        assert!(result.unwrap_err().is::<Cancelled>());
    }

    #[test]
    fn success_stream_cannot_starve_cancellation() {
        let cancel = AtomicBool::new(false);
        let mut reads = 0;
        let mut accepted_events = 0;
        let mut cancelled = false;
        // Model the sensor's wait-for-finger loop with an interrupt stream that
        // never times out. Stop after five events even in the buggy version.
        for _ in 0..5 {
            let result = wait_interrupt(&cancel, |buf| {
                reads += 1;
                if reads == 3 {
                    cancel.store(true, Ordering::SeqCst);
                }
                buf[0] = 1;
                Ok(1)
            });
            match result {
                Ok(event) => {
                    assert_eq!(event, [1]);
                    accepted_events += 1;
                }
                Err(e) => {
                    assert!(e.is::<Cancelled>());
                    cancelled = true;
                    break;
                }
            }
        }
        assert!(cancelled, "successful interrupt reads starved cancellation");
        assert_eq!(reads, 3);
        assert_eq!(accepted_events, 2);
    }

    #[test]
    fn usb_error_keeps_its_failure_type() {
        let cancel = AtomicBool::new(false);
        let result = wait_interrupt(&cancel, |_| Err(rusb::Error::NoDevice));
        assert!(matches!(
            result.unwrap_err().downcast_ref::<UsbFailure>(),
            Some(UsbFailure(rusb::Error::NoDevice))
        ));
    }
}
