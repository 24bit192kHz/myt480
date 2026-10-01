# Switching the T480 from python-validity to validity-rs

Target: this Artix T480 (s6 / s6-rc with the `s6` frontend, open-fprintd
supervised by s6, live `/etc` edits staged with `syswork`). Steps marked
`#` run as root (`ssh root@localhost` or a root shell), `$` as btw.

The package coexists with python-validity (no shared files), so the whole
switch is reversible with the s6 commands in [Rollback](#rollback). What
must never happen is **python3-validity and validity-rs up at the same
time**: both drive the same USB device, and validity-rs resets the port when
a session looks wedged, which would also kill python-validity's session. The
validity-rs run script refuses to start while python-validity's
`dbus-service` is running, but the procedure below stops it first anyway.

## 0. Baseline

```sh
# ls /run/service | wc -l                          # note this number (~73)
# s6 live status open-fprintd python3-validity
# pgrep -af 'validity'                             # only python-validity's dbus-service
```

Stop any hand-started test copy of validity-rs (for example
`target/release/validity-rs -c /root/vrs-test.conf daemon`) before going
on; `pgrep -af validity-rs` must print nothing.

`/var/lib/python-validity` stays untouched: validity-rs only reads from it.

## 1. Build the package

```sh
$ cd validity-rs/packaging
$ makepkg -f              # cargo build --release --locked, then cargo test
```

This produces `validity-rs-0.1.0-1-x86_64.pkg.tar.zst` next to the
PKGBUILD. With `tests/golden/golden.json` present the golden-vector tests run
against this sensor's data; without it they are skipped (`--nocheck` skips
all tests).

## 2. Stage /etc/validity-rs.conf with syswork

```sh
$ sudo syswork new validity-rs
$ sudo install -m644 validity-rs/packaging/validity-rs.conf.example \
      /srv/work/validity-rs/etc/validity-rs.conf
$ syswork diff validity-rs --full        # review: one new file, hwkey + hwkey_fallback active
$ sudo syswork apply validity-rs --yes --drop
```

The example pins `hwkey = T480 123456789` (coreboot's SMBIOS identity, which
the current pairing is sealed with) and `hwkey_fallback = 20L6S4VC00 PF0XXXXX`
(the stock BIOS identity). The file is inert until the daemon starts.

## 3. Install the package

```sh
# pacman -U ~btw/Projects/software/validity-rs/packaging/validity-rs-0.1.0-1-x86_64.pkg.tar.zst
# ls /run/service | wc -l                          # baseline or baseline+1
# s6 set status | grep -E 'validity|fprint'
```

Installing `/etc/s6/sv/validity-rs` fires Artix's `s6-rc-db-update` pacman
hook (`s6 repository sync && s6 set commit && s6 live install`); the
`dbus-reload` hook loads `io.github.uunicorn.Fprint.validity-rs.conf`.

**On this machine the hook's `s6 live install` fails** (2026-09-29, same bug as
2026-09-28): `s6-rc-update: fatal: unable to manage new service directories in
/run/s6-rc: Broken pipe`, and `/run/service` drops to ~4 entries. Running
services keep running, but stopped ones lose their supervisors. Check the
count right after `pacman -U`; if it collapsed, follow
[Repair](#repair-if-run-service-lost-its-links) before anything else. After the
repair the half-updated live database already contains validity-rs, so step 4
switches with `s6-rc change` and does not run `s6 live install` again.

## 4. Switch the services

```sh
# s6-rc -v2 -d change python3-validity   # its finish copies calib-data.bin to /var/lib/python-validity
# pgrep -af python-validity/dbus-service   # must print nothing

# s6 repository sync                   # new-service pitfall: repository first, then enable
# s6 set disable python3-validity
# s6 set enable validity-rs
# s6 set commit
# s6 live install                      # REQUIRED: only this switches the boot database;
                                       # safe now that the live db knows validity-rs
# ls /run/service | wc -l              # must not drop

# s6-rc -v2 -u change validity-rs      # start it in the live database
```

Order matters: `s6 set enable` on a service the repository does not know yet
made `s6 live install` fail on 2026-09-28 and left `/run/service` without its
s6-rc links. `s6 set commit` alone does NOT change what boots (2026-09-29: a
rollback done with commit only came back up on the other service after a
reboot); `s6 live install` is what switches `/etc/s6/rc/compiled`. It only
breaks for services the live database does not know yet, so after the step-3
repair it is safe.
`s6 set disable` keeps python3-validity compiled into the database, so it can
still be started by hand for the rollback.

### First start: calibration import

On its first start validity-rs finds no `/var/lib/validity-rs/calib-data.bin`
and imports `/var/lib/python-validity/calib-data.bin` (falling back to
`/var/run/python-validity/`), after checking it against the clean-slate image
on the sensor's flash, and writes it to `/var/lib/validity-rs/`. Log line:
`importing calibration data from /var/lib/python-validity/calib-data.bin`.
If the flash has no matching calibration it recalibrates instead (a few
seconds; keep fingers off the sensor). The fwext firmware file
(`6_07f_lenovo_mis_qm.xpfwext`) is only needed when the sensor has lost its
firmware extension (after a factory reset); it is looked up in
`/var/lib/validity-rs` and then the python-validity directories.

## 5. Verify

```sh
# s6 live status validity-rs python3-validity open-fprintd   # validity-rs up, python3-validity down
# s6-svstat /run/service/validity-rs     # "up (pid N) S seconds"; re-run after 15 s: same pid
# pgrep -af 'validity'                   # exactly one: /usr/bin/validity-rs daemon
# s6-tai64nlocal < /run/uncaught-logs/current | grep -E 'host identity|calibration|sensor ready|serving|open-fprintd' | tail
```

Expected log lines (catch-all logger, no service prefix):

```
INFO  pairing unlocked with host identity "T480" / "123456789"
INFO  importing calibration data from /var/lib/python-validity/calib-data.bin   (first start only)
INFO  sensor ready in 6xx ms
INFO  serving io.github.uunicorn.Fprint.Device at /io/github/uunicorn/Fprint/Device
INFO  registered with open-fprintd
```

```sh
# ls -l /var/lib/validity-rs/
# cmp /var/lib/validity-rs/calib-data.bin /var/lib/python-validity/calib-data.bin && echo same
# dbus-send --system --print-reply --dest=net.reactivated.Fprint \
      /net/reactivated/Fprint/Manager net.reactivated.Fprint.Manager.GetDevices

$ fprintd-list btw                  # the fingers enrolled under python-validity
$ fprintd-verify btw                # touch: verify-match
```

Then the real paths:

- Lock the screen (slock-ly) and unlock with a finger; also unlock with the
  password once.
- Suspend and resume (`loginctl suspend`), then unlock with a finger again.
  `/etc/elogind/system-sleep/05-fprintd` drives open-fprintd's Suspend/Resume,
  which reach validity-rs; the log shows `resumed in N ms`.

Standalone diagnostics need the sensor to themselves:

```sh
# s6 live stop validity-rs && validity-rs info; s6 live start validity-rs
```

## Rollback

```sh
# s6-rc -v2 -d change validity-rs      # cancels any capture, reboots the sensor, releases USB
# pgrep -af validity-rs                # must print nothing
# s6 repository sync
# s6 set disable validity-rs
# s6 set enable python3-validity
# s6 set commit
# s6 live install                      # switches the boot database (see step 4)
# ls /run/service | wc -l              # must not drop
# rm -f /var/run/python-validity/backoff   # python-validity refuses >10 starts/min
# s6-rc -v2 -u change python3-validity
$ fprintd-verify btw
```

This works because validity-rs never re-pairs a paired sensor, and the
pairing stays sealed with `T480 123456789`, which python-validity reads from
the live DMI under coreboot. Fingers enrolled with validity-rs use the same
SID scheme (`S-1-5-21-111111111-1111111111-1111111111-<uid>`), so
python-validity sees them.

`/etc/validity-rs.conf` is inert while validity-rs is disabled; leave it or
delete it as root (syswork apply does not propagate deletions). To remove the
package as well, disable the service first (above), then
`pacman -R validity-rs`; the `s6-rc-remove` / `s6-rc-db-update` hooks sync the
repository again. Check `ls /run/service | wc -l` afterwards.

## Later: removing python-validity

Once validity-rs has run through a few suspend cycles and lock/unlock:

```sh
# install -Dm644 /var/lib/python-validity/6_07f_lenovo_mis_qm.xpfwext /var/lib/validity-rs/
# s6 set mask python3-validity && s6 set commit && s6 live install   # its run script needs python-validity
# ls /run/service | wc -l
# pacman -R python-validity            # not -Rs; open-fprintd and fprintd stay (validity-rs depends on open-fprintd)
# dbus-send --system --print-reply --dest=net.reactivated.Fprint \
      /net/reactivated/Fprint/Manager net.reactivated.Fprint.Manager.GetDevices
```

`/etc/s6/sv/python3-validity` is a local directory (no package owns it), so
pacman leaves it in place; masking keeps it out of the database. The D-Bus
policy python-validity shipped is duplicated by validity-rs's own file.
`/usr/lib/udev/rules.d/60-python-validity.rules` goes away with the package
(it only called systemctl); `/etc/udev/rules.d/99-validity-no-autosuspend.rules`
stays and keeps the sensor out of USB autosuspend.

## Upgrading

```sh
$ cd validity-rs/packaging && makepkg -f
# pacman -U validity-rs-*.pkg.tar.zst
# s6 live restart validity-rs
```

## Repair if /run/service lost its links

The 2026-09-28 failure mode: `s6 live install` fails with
`s6-rc-update: unable to manage new service directories ... Broken pipe` and
`/run/service` keeps almost none of its s6-rc links; stopped services lose
their supervisors. Do **not** run `s6-svscanctl -an` in that state (it would
take down NetworkManager, sshd and the X session on tty1). Instead relink
every compiled service directory, touching `down` first where `s6-svok`
fails, then rescan:

```sh
# live=$(readlink -f /run/s6-rc)
# for d in "$live"/servicedirs/*; do n=${d##*/}; [ -e "/run/service/$n" ] && continue; \
      s6-svok "$d" 2>/dev/null || touch "$d/down"; ln -s "$d" "/run/service/$n"; done
# s6-svscanctl -a /run/service
# ls /run/service | wc -l
```
