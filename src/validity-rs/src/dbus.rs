//! io.github.uunicorn.Fprint.Device for open-fprintd, wire-compatible with python-validity.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use zbus::blocking::Connection;
use zbus::names::BusName;
use zbus::zvariant::ObjectPath;
use zbus::{DBusError, interface};

use crate::blobs;
use crate::config::Config;
use crate::device::Device;
use crate::sensor::{Progress, is_cancelled, is_usb_failure};

pub const PATH: &str = "/io/github/uunicorn/Fprint/Device";
pub const IFACE: &str = "io.github.uunicorn.Fprint.Device";
const MANAGER: &str = "net.reactivated.Fprint";
const MANAGER_PATH: &str = "/net/reactivated/Fprint/Manager";
const MANAGER_IFACE: &str = "net.reactivated.Fprint.Manager";

#[derive(Debug, DBusError)]
#[zbus(prefix = "net.reactivated.Fprint.Error")]
pub enum FprintError {
    #[zbus(error)]
    ZBus(zbus::Error),
    NoEnrolledPrints(String),
    AlreadyInUse(String),
    Internal(String),
    InvalidFingername(String),
}

fn internal(e: anyhow::Error) -> FprintError {
    FprintError::Internal(format!("{e:#}"))
}

pub struct Shared {
    dev: Mutex<Option<Device>>,
    cancel: Arc<AtomicBool>,
    /// Set when the daemon itself aborts the running operation (suspend,
    /// shutdown): the client then gets a final "*-disconnected" status instead
    /// of waiting out its PAM timeout.
    daemon_abort: AtomicBool,
    /// Once set, no new operation may start (it would undo the abort).
    shutting_down: AtomicBool,
    cfg: Config,
    conn: OnceLock<Connection>,
}

impl Shared {
    pub fn new(dev: Device, cfg: Config) -> Arc<Shared> {
        Arc::new(Shared {
            cancel: dev.cancel.clone(),
            daemon_abort: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            dev: Mutex::new(Some(dev)),
            cfg,
            conn: OnceLock::new(),
        })
    }

    /// Wait briefly for the sensor; a running capture holds it until it finishes or is cancelled.
    fn lock(&self, wait: Duration) -> Result<MutexGuard<'_, Option<Device>>, FprintError> {
        let deadline = Instant::now() + wait;
        loop {
            match self.dev.try_lock() {
                Ok(g) => return Ok(g),
                Err(std::sync::TryLockError::Poisoned(p)) => return Ok(p.into_inner()),
                Err(std::sync::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => return Err(FprintError::AlreadyInUse("sensor busy".into())),
            }
        }
    }

    fn emit(&self, signal: &str, body: &(impl serde::Serialize + zbus::zvariant::DynamicType)) {
        if let Some(c) = self.conn.get()
            && let Err(e) = c.emit_signal(None::<BusName<'_>>, PATH, IFACE, signal, body) {
                warn!("emit {signal}: {e}");
            }
    }

    /// Run a short sensor operation; on a USB failure reconnect once and retry.
    fn with_dev<T>(&self, wait: Duration, f: impl Fn(&mut Device) -> Result<T>) -> Result<T, FprintError> {
        let mut g = self.lock(wait)?;
        if g.is_none() {
            self.recover(&mut g);
        }
        match f(g.as_mut().unwrap()) {
            Err(e) if is_usb_failure(&e) => {
                warn!("{e:#}; reconnecting the sensor");
                self.recover(&mut g);
                f(g.as_mut().unwrap()).map_err(internal)
            }
            r => r.map_err(internal),
        }
    }

    /// Replace a device whose USB session died. Exits when the sensor is gone for good.
    fn recover(&self, slot: &mut Option<Device>) {
        if let Some(d) = slot.take() {
            d.close();
        }
        match Device::open(&self.cfg, self.cancel.clone()) {
            Ok(d) => *slot = Some(d),
            Err(e) => {
                error!("cannot reopen the sensor: {e:#}; exiting for the supervisor to restart us");
                std::process::exit(1);
            }
        }
    }

    /// Run `f` on a worker thread holding the sensor; results go out as signals.
    fn spawn(self: &Arc<Self>, what: &'static str, f: impl FnOnce(&Arc<Shared>, &mut Option<Device>) + Send + 'static) {
        let me = self.clone();
        std::thread::Builder::new()
            .name(what.into())
            .spawn(move || {
                let mut g = me.dev.lock().unwrap_or_else(|p| p.into_inner());
                f(&me, &mut g);
            })
            .expect("spawn worker");
    }

    /// Abort the running operation on the daemon's behalf.
    fn abort(&self) {
        self.daemon_abort.store(true, Ordering::SeqCst);
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Start of a client operation: forget earlier cancels and aborts.
    fn arm(&self) -> Result<(), FprintError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(FprintError::Internal("daemon is shutting down".into()));
        }
        self.daemon_abort.store(false, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn cancelled(&self, what: &str) {
        if self.daemon_abort.load(Ordering::SeqCst) {
            info!("{what} aborted by the daemon; telling the client");
            let (sig, status) = if what == "verify" {
                ("VerifyStatus", "verify-disconnected")
            } else {
                ("EnrollStatus", "enroll-disconnected")
            };
            self.emit(sig, &(status, true));
        } else {
            debug!("{what} cancelled");
        }
    }

    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        self.abort();
        let mut g = self.dev.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(d) = g.take() {
            d.close();
        }
    }
}

pub struct FprintDevice {
    pub shared: Arc<Shared>,
}

#[interface(name = "io.github.uunicorn.Fprint.Device")]
impl FprintDevice {
    #[zbus(name = "Suspend")]
    fn suspend(&self) {
        debug!("Suspend");
        self.shared.abort();
    }

    #[zbus(name = "Resume")]
    fn resume(&self) -> Result<(), FprintError> {
        debug!("Resume");
        let mut g = self.shared.lock(Duration::from_secs(5))?;
        let t = Instant::now();
        let res = match g.take() {
            Some(d) => d.reinit(),
            None => Device::open(&self.shared.cfg, self.shared.cancel.clone()),
        };
        match res {
            Ok(d) => {
                *g = Some(d);
                info!("resumed in {} ms", t.elapsed().as_millis());
                Ok(())
            }
            Err(e) => {
                error!("resume failed: {e:#}");
                Err(internal(e))
            }
        }
    }

    #[zbus(name = "ListEnrolledFingers")]
    fn list_enrolled_fingers(&self, user: &str) -> Result<Vec<String>, FprintError> {
        debug!("ListEnrolledFingers {user}");
        let sid = self.shared.cfg.sid_for(user).map_err(internal)?;
        let usr = self.shared.with_dev(Duration::from_secs(3), |d| d.lookup_user(&sid))?;
        Ok(usr.map(|u| u.fingers.iter().map(|f| blobs::finger_name(f.subtype).to_string()).collect()).unwrap_or_default())
    }

    #[zbus(name = "DeleteEnrolledFingers")]
    fn delete_enrolled_fingers(&self, user: &str) -> Result<(), FprintError> {
        info!("DeleteEnrolledFingers {user}");
        let sid = self.shared.cfg.sid_for(user).map_err(internal)?;
        self.shared.with_dev(Duration::from_secs(3), |d| match d.lookup_user(&sid)? {
            Some(u) => d.del_record(u.dbid),
            None => Ok(()),
        })
    }

    #[zbus(name = "VerifyStart")]
    fn verify_start(&self, user: &str, finger: &str) -> Result<(), FprintError> {
        debug!("VerifyStart {user} {finger}");
        let sid = self.shared.cfg.sid_for(user).map_err(internal)?;
        let usr = self.shared.with_dev(Duration::from_secs(3), |d| d.lookup_user(&sid))?;
        let Some(usr) = usr.filter(|u| !u.fingers.is_empty()) else {
            return Err(FprintError::NoEnrolledPrints("No enrolled prints found".into()));
        };
        self.shared.emit("VerifyFingerSelected", &("any",));
        self.shared.arm()?;
        let t0 = Instant::now();
        self.shared.spawn("verify", move |sh, slot| {
            if slot.is_none() {
                sh.recover(slot);
            }
            let dev = slot.as_mut().unwrap();
            let res = dev.identify(&mut |p| {
                if let Progress::RetryScan(e) = p {
                    debug!("verify retry: {e}");
                    sh.emit("VerifyStatus", &("verify-retry-scan", false));
                }
            });
            match res {
                Ok(Some((uid, sub, _))) if uid == usr.dbid as u32 => {
                    info!("verify: match ({}) in {} ms", blobs::finger_name(sub), t0.elapsed().as_millis());
                    sh.emit("VerifyStatus", &("verify-match", true));
                }
                Ok(m) => {
                    info!("verify: no match ({m:?})");
                    sh.emit("VerifyStatus", &("verify-no-match", true));
                }
                Err(e) if is_cancelled(&e) => sh.cancelled("verify"),
                Err(e) => {
                    error!("verify failed: {e:#}");
                    // Reconnect before reporting, so an immediate retry finds the sensor ready.
                    if is_usb_failure(&e) {
                        sh.recover(slot);
                    }
                    sh.emit("VerifyStatus", &("verify-no-match", true));
                }
            }
        });
        Ok(())
    }

    #[zbus(name = "Cancel")]
    fn cancel(&self) {
        debug!("Cancel");
        self.shared.cancel.store(true, Ordering::SeqCst);
    }

    #[zbus(name = "EnrollStart")]
    fn enroll_start(&self, user: &str, finger: &str) -> Result<(), FprintError> {
        info!("EnrollStart {finger} for {user}");
        let sid = self.shared.cfg.sid_for(user).map_err(internal)?;
        let Some(subtype) = blobs::finger_id(finger) else {
            return Err(FprintError::InvalidFingername(format!("unknown finger {finger}")));
        };
        self.shared.arm()?;
        let finger = finger.to_string();
        self.shared.spawn("enroll", move |sh, slot| {
            if slot.is_none() {
                sh.recover(slot);
            }
            let dev = slot.as_mut().unwrap();
            let res = dev.enroll(&sid, subtype, &mut |p| match p {
                Progress::StagePassed => sh.emit("EnrollStatus", &("enroll-stage-passed", false)),
                Progress::RetryScan(e) => {
                    debug!("enroll retry: {e}");
                    sh.emit("EnrollStatus", &("enroll-retry-scan", false))
                }
            });
            match res {
                Ok(rec) => {
                    info!("enrolled {finger} (record {rec})");
                    sh.emit("EnrollStatus", &("enroll-completed", true));
                }
                Err(e) if is_cancelled(&e) => sh.cancelled("enroll"),
                Err(e) => {
                    error!("enroll failed: {e:#}");
                    if is_usb_failure(&e) {
                        sh.recover(slot);
                    }
                    sh.emit("EnrollStatus", &("enroll-failed", true));
                }
            }
        });
        Ok(())
    }

    #[zbus(name = "RunCmd")]
    fn run_cmd(&self, cmd: &str) -> Result<String, FprintError> {
        let bytes = hex::decode(cmd.trim()).map_err(|e| FprintError::Internal(e.to_string()))?;
        Ok(hex::encode(self.shared.with_dev(Duration::from_secs(3), |d| d.app(&bytes))?))
    }

    #[zbus(signal, name = "VerifyStatus")]
    async fn verify_status(e: &zbus::object_server::SignalEmitter<'_>, result: &str, done: bool) -> zbus::Result<()>;

    #[zbus(signal, name = "VerifyFingerSelected")]
    async fn verify_finger_selected(e: &zbus::object_server::SignalEmitter<'_>, finger: &str) -> zbus::Result<()>;

    #[zbus(signal, name = "EnrollStatus")]
    async fn enroll_status(e: &zbus::object_server::SignalEmitter<'_>, result: &str, done: bool) -> zbus::Result<()>;
}

fn register(conn: &Connection) {
    let path = ObjectPath::from_static_str_unchecked(PATH);
    match conn.call_method(Some(MANAGER), MANAGER_PATH, Some(MANAGER_IFACE), "RegisterDevice", &(path,)) {
        Ok(_) => info!("registered with open-fprintd"),
        Err(e) => warn!("RegisterDevice: {e} (will retry when open-fprintd appears)"),
    }
}

/// Serve the device on the system bus and keep it registered with open-fprintd.
pub fn serve(shared: Arc<Shared>) -> Result<()> {
    let conn = zbus::blocking::connection::Builder::system()?
        .serve_at(PATH, FprintDevice { shared: shared.clone() })?
        .build()
        .context("connect to the system bus")?;
    let _ = shared.conn.set(conn.clone());

    let dbus = zbus::blocking::fdo::DBusProxy::new(&conn)?;
    let changes = dbus.receive_name_owner_changed_with_args(&[(0, MANAGER)])?;
    register(&conn);
    std::thread::Builder::new().name("owner-watch".into()).spawn(move || {
        for sig in changes {
            if let Ok(args) = sig.args() {
                if args.new_owner().is_some() {
                    info!("open-fprintd is back ({:?}); registering", args.new_owner());
                    register(&conn);
                } else {
                    info!("open-fprintd went away");
                }
            }
        }
    })?;
    Ok(())
}
