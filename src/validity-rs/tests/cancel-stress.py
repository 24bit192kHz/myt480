import dbus, sys, time
n = int(sys.argv[1]); hold = float(sys.argv[2])
bus = dbus.SystemBus()
mgr = dbus.Interface(bus.get_object('net.reactivated.Fprint', '/net/reactivated/Fprint/Manager'), 'net.reactivated.Fprint.Manager')
dev = dbus.Interface(bus.get_object('net.reactivated.Fprint', mgr.GetDefaultDevice()), 'net.reactivated.Fprint.Device')
bad = 0
for i in range(n):
    try:
        dev.Claim(sys.argv[3]); dev.VerifyStart('any'); time.sleep(hold); dev.VerifyStop(); dev.Release()
        dev.Claim(sys.argv[3]); f = list(dev.ListEnrolledFingers(sys.argv[3])); dev.Release()
    except Exception as e:
        bad += 1; print('cycle', i, 'error', str(e)[:120])
        try: dev.Release()
        except Exception: pass
print('cycles with errors: %d/%d' % (bad, n))
