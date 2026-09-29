# Verify through open-fprintd, then suspend (or SIGTERM the daemon: argv[2]=kill)
# mid-verify; the client must get verify-disconnected.
import dbus, dbus.mainloop.glib, subprocess, sys, time
from gi.repository import GLib
dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
bus = dbus.SystemBus()
mgr = dbus.Interface(bus.get_object('net.reactivated.Fprint', '/net/reactivated/Fprint/Manager'), 'net.reactivated.Fprint.Manager')
path = mgr.GetDefaultDevice()
dev = dbus.Interface(bus.get_object('net.reactivated.Fprint', path), 'net.reactivated.Fprint.Device')
got = []
loop = GLib.MainLoop()
def status(result, done):
    got.append((str(result), bool(done)))
    if done:
        loop.quit()
bus.add_signal_receiver(status, 'VerifyStatus', 'net.reactivated.Fprint.Device', path=path)
dev.Claim(sys.argv[1]); dev.VerifyStart('any')
t = time.time()
action = ['/usr/lib/open-fprintd/suspend.py'] if len(sys.argv) < 3 else ['sh', '-c', 'kill -TERM $(cat /root/vrs.pid)']
GLib.timeout_add(1000, lambda: subprocess.Popen(action) and False)
GLib.timeout_add(8000, loop.quit)
loop.run()
print('statuses:', got, 'after %.0f ms' % ((time.time() - t) * 1000))
try: dev.Release()
except Exception as e: print('release:', e)
if len(sys.argv) < 3:
    subprocess.run(['/usr/lib/open-fprintd/resume.py'])
    dev.Claim(sys.argv[1]); print('fingers after resume:', [str(f) for f in dev.ListEnrolledFingers(sys.argv[1])]); dev.Release()
