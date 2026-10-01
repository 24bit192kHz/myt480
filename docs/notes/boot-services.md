# Boot + Service Supervision Notes (Artix s6/s6-rc, T480)

> **2026-09-28:** partly superseded, see [2026-09-28-overhaul.md](2026-09-28-overhaul.md).

Date: 2026-09-19. Host: Artix Linux, ThinkPad T480. Read-only recon; nothing modified.
PID1: `s6-svscan -X3 -- /run/service` (confirmed via `ps -p 1`). NOT systemd/runit.

## 1. Init system overview (s6-svscan → s6-rc)

- PID1 is `s6-svscan` scanning `/run/service` (69 entries live).
- Service manager is **s6-rc**: compiled DB lives in `/etc/s6/rc/` (`compiled -> .current:@400000006aac06a52f8abd1a:n2P5KD`
  plus 4 timestamped `compiled-<n>` dirs). Live instantiated dirs are symlinked from
  `/run/service/<name> -> /run/s6-rc:s6-rc-init:<id>/servicedirs/<name>`.
- Source definitions: `/etc/s6/sv/<name>/` (type `longrun`|`oneshot`|`bundle`, `run`, `dependencies.d/`,
  `producer-for`, `consumer-for`, `notification-fd`, optional `down` flag file).
- Bundle sources: `/etc/s6/sv/boot/` (type `bundle`, contents.d: `misc misc-essential mount setup udev`)
  and `/etc/s6/sv/getty/` (type `bundle`, contents.d: `tty1..tty6 ttyS`).
- Admin (early, pre-s6-rc) services: `/etc/s6/adminsv/` =
  `default-<timestamp>/ mount-filesystems/ network/ rc-local/` (each with `type`, `contents.d/`,
  `dependencies.d/`, `up`/`down` scripts).
- Per-service config: `/etc/s6/config/*.conf` (envfile-sourced by `run` scripts).
- `s6-rc -a list` currently shows **48 active** services (see §3). `s6-svstat /run/service/<name>`
  as non-root fails with `Permission denied` — status checks need root or use `s6-rc -a list`.
- Tools in PATH: `/usr/bin/s6-rc /usr/bin/s6-svstat /usr/bin/s6-linux-init`.
  (`s6-rc-bundle-update` / `s6-db-compile` NOT in user PATH; Artix wraps them — see §8.)

## 2. Service inventory (`ls -1 /etc/s6/sv/` = ~95 entries)

| Name | What | Stock/Custom | Boot state |
|---|---|---|---|
| thermald | thermal daemon (custom wrapper, see §3) | CUSTOM | up (thinkfan and throttled, formerly parked with down files, were removed 2026-10-01) |
| validity-rs | Rust fingerprint daemon for the 06cb:009a (`/usr/bin/validity-rs daemon`, pkg validity-rs) | CUSTOM | up, deps: open-fprintd (since 2026-09-29) |
| python3-validity | Validity fingerprint dbus-service + fw restore | CUSTOM | usable, disabled 2026-09-29 (rollback for validity-rs) |
| open-fprintd | `/usr/lib/open-fprintd/open-fprintd --debug` | CUSTOM | up |
| NetworkManager-{log,srv} | `NetworkManager -d` | stock | up |
| sshd-{log,srv} | `sshd -D -e`, keygen in run | stock | up |
| dbus-{log,srv} | system bus, `notification-fd`, producer-for dbus-log | stock | up |
| sddm-{log,srv} | display manager | stock | installed, NOT in live list (tty1 autologin used instead) |
| elogind | `/usr/lib/elogind/elogind` | stock | up |
| metalog / cronie | `metalog`, `crond -n` | stock | up |
| acpid / udevd / dmesg + -log | kernel/acpi/udev plumbing | stock | up |
| ntpd-{log,srv} | openntpd `ntpd -d`, pipeline ntpd-srv→ntpd-log, s6-log `n3 s2000000 T` | stock | up (enabled 2026-09-21) |
| dhcpcd / wpa_supplicant / avahi / bluetoothd / cupsd | optional net/BT/print | stock | installed, not active |
| oneshots (mount-*, binfmt, hostname, hwclock, locale, modules, net-lo, sysctl, tmpfiles-*, udevadm, …) | boot sequencing | stock | done (oneshot) |
| lvm2*/dmeventd/lvmpolld/mdadm/rpcbind/nfs*/rsyncd/haveged | storage/net extras | stock | installed; lvm/dm/mdadm have `down` files |

Stock-disabled via `down` file (present but not started): `dmeventd-*`, `hwclock` (222B, reason text
inside), `lvm2-monitor`, `lvm2-pvscan`, `lvmpolld-*`, `mount-net`, `random-seed`, `swap`,
plus all CUSTOM down-files above.

## 3. Custom service details (exact `run` lines)

- `/etc/s6/sv/thermald/`: `type=longrun`, no deps file, NO down file (enabled).
  `run`: `#!/bin/sh` + `exec /usr/local/bin/thermald-t480 -c /etc/thermald.conf`
  (custom binary; owns TCC via MSR 0x1A2 trip 90 per modprobe comment).
  `run`: `export HOME=/home/btw USER=btw LOGNAME=btw` +
- `/etc/s6/sv/python3-validity/`: `type=longrun`, enabled, `dependencies.d/open-fprintd` (empty).
  `run`: `mkdir -p /var/run/python-validity /var/lib/python-validity`; copies persistent
  `/var/lib/python-validity/*` → `/var/run/python-validity/` if missing; then
  `exec /usr/lib/python-validity/dbus-service --debug`.
- `/etc/s6/sv/validity-rs/` (from pkg validity-rs, built from ~/Projects/software/validity-rs/packaging):
  `type=longrun`, enabled, `dependencies.d/open-fprintd`. `run`: refuses to start while
  python-validity's dbus-service runs, then `exec /usr/bin/validity-rs daemon` (stderr → catch-all log).
  Config `/etc/validity-rs.conf` pins the pairing identity (`hwkey = T480 123456789`, fallback
  `20L6S4VC00 PF0XXXXX`); state in `/var/lib/validity-rs` (calib-data.bin imported from python-validity).
  2026-09-30: switched back to validity-rs (python3-validity disabled). `s6 live install` failed again
  with the Broken-pipe `s6-rc-update` error even though the live db knew both services; after the
  relink repair the live db was the new compiled set, so the boot db was switched by repointing
  `/etc/s6/rc/compiled` at it by hand. The relink left svscan respawning `s6-supervise` every second
  for 23 idle services ("another instance of s6-supervise is already running"); fixed per service with
  `touch down; s6-svc -x` (all were down and not in `s6-rc -a list`).
  python-validity enroll of an already-enrolled finger fails with `04c3`; `fprintd-delete` first.
  Switch-over/rollback: packaging/INSTALL.md in the source tree.
- `/etc/s6/sv/open-fprintd/`: `type=longrun`, enabled. `run`: `exec /usr/lib/open-fprintd/open-fprintd --debug`.
  `run`: `export NO_COLOR=true` +

Stock `run` flavor is execline (`#!/bin/execlineb -P`): e.g. `sshd-srv` runs `ssh-keygen -A`,
touches `/var/log/lastlog`, sources `/etc/s6/config/openssh.conf`, `exec /usr/bin/sshd -D -e`;
`dbus-srv` has `notification-fd` + `producer-for: dbus-log`. Logging pattern: every `-srv` has
`producer-for: <name>-log`, and the `-log` dir runs `s6-log` with `DIRECTIVES="n3 s2000000 T"`.

## 4. Boot chain (grub → RT kernel → fstab)

- `/boot/`: `grub/ memtest86+/ System.map-t480 config-t480 initramfs-linux-rt.img (17.0M)`
  `initramfs-linux.img (17.1M) intel-ucode.img (14.6M) vmlinuz-linux vmlinuz-linux-rt (16.4M each)`.
- `/boot/grub/grub.cfg` is `600 root:root` — NOT readable as user (grep denied); backup
  `grub.cfg.bak-hibfix` exists. `/boot/grub/grubenv` (1K), `fonts/ i386-pc/ locale/ themes/`.
- `/etc/default/grub`: `GRUB_DEFAULT="gnulinux-advanced-…>gnulinux-linux-rt-advanced-…"` → **RT kernel default**;
  `TIMEOUT=0`, `TIMEOUT_STYLE=hidden`; `GRUB_CMDLINE_LINUX_DEFAULT='quiet loglevel=0 fbcon=nodefer
  vt.global_cursor_default=0 printk.devkmsg=off nowatchdog nmi_watchdog=0 rd.udev.log_level=0
  8250.nr_uarts=0 iomem=relaxed thinkpad_acpi.force_load=1 thinkpad_acpi.fan_control=1
  pcie_aspm=performance nvme_core.io_timeout=30 nvme_core.admin_timeout=30 init_on_alloc=0
  init_on_free=0 mitigations=off usb-storage.delay_use=0 usbcore.autosuspend=-1
  psmouse.synaptics_intertouch=1 i915.enable_psr=1 i915.enable_fbc=1
  nvme_core.default_ps_max_latency_us=5500 resume=UUID=<swap UUID>'`; `GRUB_CMDLINE_LINUX="net.ifnames=0"`;
  `PRELOAD_MODULES="part_msdos nvme"`, `GFXPAYLOAD_LINUX="text"`, os-prober + recovery disabled,
  early initrd stock `intel-ucode`.
- `/proc/cmdline` matches: `BOOT_IMAGE=/boot/vmlinuz-linux-rt root=UUID=<root UUID> rw net.ifnames=0 …`.
- `/etc/fstab` (644): `UUID=<root UUID> / ext4 defaults,noatime 0 1`;
  `UUID=<swap UUID> swap pri=100`; `tmpfs /tmp mode=1777`;
  `192.168.0.23:/my-zfs /mnt/my-zfs nfs noauto,nofail,soft,timeo=10,retrans=2,_netdev 0 0`;
  `/swapfile none swap pri=10`.
- `/etc/mkinitcpio.conf`: `MODULES=(nvme i915)`, `BINARIES=() FILES=()`,
  `HOOKS=(base udev autodetect modconf kms block filesystems resume)` — no microcode (via grub),
  no encrypt/lvm/mdadm hooks (matches disabled dm/mdadm services).

## 5. `/etc/modprobe.d/` rationale table

| File | Content | Rationale |
|---|---|---|
| `30-defer-lan.conf` (43B) | `blacklist e1000e` + `install e1000e /bin/false` | defer wired LAN (no comment in file) |
| `31-defer-media.conf` (140B) | blacklist `uvcvideo btusb pcspkr bluetooth bnep`; `install bluetooth/bnep /bin/false` | defer camera + BT (no comment) |
| `32-disable-tcc.conf` (144B) | blacklist+false `intel_tcc_cooling` | "tcc owned by thermald-t480 via MSR 0x1A2 trip 90; driver probe ~170ms wasted" |
| `33-disable-wwan.conf` (289B) | blacklist+false `qrtr qrtr_mhi qrtr_smd qrtr_tun wwan` | "no WWAN SIM, ModemManager unused; qrtr probe ~140ms + daemon wasted" |
| `34-disable-dm.conf` (336B) | blacklist+false `dm_mod dm_snapshot dm_mirror dm_thin_pool` | "no mapper devices (dmsetup empty, fstab UUID-only); crypttab comments-only. saves dm_mod load ~30ms; revert: rm file + mkinitcpio -P" |
| `thinkpad_acpi.conf` (36B) | `options thinkpad_acpi fan_control=1` | enable fan control (mirrors kernel cmdline) |

## 6. sysctl / udev customizations

- `/etc/sysctl.d/`: single file `99_magic_sysrq.conf` → `kernel.sysrq=1`.
- `/etc/udev/rules.d/`:
  - `77-mm-huawei-net-port-types.rules -> /dev/null`, `80-mm-candidate.rules -> /dev/null`,
    `80-net-name-slot.rules -> /dev/null` — mask ModemManager probes + slot naming (pairs with
    `net.ifnames=0` and WWAN disable).
  - `99-uinput.rules` (76B): `KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"`.
  - `99-validity-no-autosuspend.rules` (94B): `SUBSYSTEM=="usb", ATTRS{idVendor}=="06cb",
    ATTRS{idProduct}=="009a", ATTR{power/control}="on"` — Validity 009a never autosuspends
    (required for python3-validity/open-fprintd).
  - `99-wifi-unblock.rules` (146B): `SUBSYSTEM=="rfkill", ATTR{type}=="wlan",
    RUN+="/usr/bin/rfkill unblock wifi"` — auto-unblock wifi on rfkill appearance.
- `/etc/s6/config/` highlights: `openssh.conf OPTS=""`; `hwclock.conf HARDWARECLOCK=UTC`;
  `dbus.conf DBUS_UUIDGEN="no"`; `tty1.conf SPAWN="yes" ARGS="-a btw" GETTY="agetty"`
  (tty1 autologins `btw` for startx — why sddm is installed but inactive).

## 7. Live state (`s6-rc -a list` = 51, 2026-09-21: +ntpd-log/ntpd-srv)

Up longruns: `NetworkManager acpid cronie dbus dmesg elogind metalog ntpd open-fprintd validity-rs
`s6rc-fdholder`). Up oneshots: `mount-net binfmt cleanup console-setup hostname hwclock locale modules
mount-sysfs mount-tmpfs net-lo network-detection random-seed rc-local remount-root swap sysctl sysusers
tmpfiles-setup kmod-static-nodes mount-cgroups mount-devfs mount-procfs tmpfiles-dev udevadm`.
sddm, bluetoothd, cupsd, dhcpcd, avahi, haveged, mdadm, rpcbind/nfs.

## 8. How to add / enable / disable a service (Artix s6-rc)

- Add: create `/etc/s6/sv/<name>/` with `type` (`longrun`|`oneshot`|`bundle`) + `run` (executable),
  optional `dependencies.d/<dep>` (empty file per dep), `producer-for`/`consumer-for` for pipelines,
  `notification-fd` if readiness-aware. Custom shell-style `run` works (all CUSTOM services here use
  `#!/bin/sh`); stock uses execline `#!/bin/execlineb -P`.
- Bundle membership = boot enablement: add/remove empty flag file under
  `/etc/s6/sv/boot/contents.d/<name>` (system) or `/etc/s6/sv/getty/contents.d/` (gettys), then
  recompile + switch the live DB (Artix wrapper around `s6-db-compile` + `s6-rc-update`;
  requires root — the exact wrapper was not in user PATH, check `artix-s6-scripts` docs on the box).
- `down` file semantics: empty `/etc/s6/sv/<name>/down` present ⇒ supervised but NOT auto-started
  Remove `down` to auto-start next boot; `touch down` to stop auto-start.
- Runtime (root): `s6-rc -u change <name>` start, `s6-rc -d change <name>` stop,
  `s6-rc -a list` inventory, `s6-svstat /run/service/<name>` status,
  `s6-rc -a change` / reboot to apply DB changes. Logs via `s6-log` dirs (`-log` services,
  `DIRECTIVES` in `/etc/s6/config/<name>.conf`).
- Boot knobs: edit `/etc/default/grub` then `grub-mkconfig -o /boot/grub/grub.cfg` (root; file is 600);
  `resume=UUID=` must match swap partition for hibernation; `mkinitcpio -P` after changing
  `MODULES`/`HOOKS` or removing `34-disable-dm.conf`.

## 9. Changelog — 2026-09-21: ntpd enabled (Artix official path)

- Was: `ntpd-srv`/`ntpd-log` compiled but in NO bundle → `s6-svstat` = `down (not started yet)`;
  stray unsupervised `ntpd -s` (pid 19727, ppid 1, from 11:24) held `/var/run/ntpd.sock`;
  clock `unsynced`, offset ~0.9s.
- Did (as root via `ssh root@localhost`): `kill 19727`, then `artix-service enable ntpd`
  (= `s6 set enable --pull-dependencies ntpd` + `repository sync` + `set commit` + `live install`),
  then `artix-service start ntpd` (= `s6-rc -u change ntpd`).
- Verified: `ntpd-log ntpd-srv` in `active/` set and `s6-rc -a list`; `s6-svstat` = `up (pid 3264)`;
  log `ntp engine ready`, constraints answering (quad9 9.9.9.9 offset 0.05ms, google ~-0.46ms),
  2/6 peers valid (cloudflare + one arch-pool v4; IPv6 pool peers `not valid`, no working v6 here);
  `/var/log/ntpd/current` logging as s6log. Clock still converging (`constraint offset -1s`,
  slew in progress — re-check with `ntpctl -s status` until `clock synced`).
- Note: §8's "contents.d + recompile" description is the raw-s6 mechanism; the Artix-official
  wrapper is `artix-service enable|start <name>` (see `/usr/bin/artix-service`).

- 2026-10-01: thinkfan and throttled removed. Boot db recompiled with `s6 set commit` and repointed by hand to `/etc/s6/rc/compiled-1790823936215054425` (no `s6 live install`); the live db keeps the two parked entries until the next boot.
