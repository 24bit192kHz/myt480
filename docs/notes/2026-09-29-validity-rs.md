# 2026-09-29 — fingerprint: python-validity → validity-rs

- New Rust daemon `validity-rs` (source ~/Projects/software/validity-rs, MIT, derived from
  python-validity) replaces python3-validity behind open-fprintd; same D-Bus interface and SIDs,
  so fprintd, pam_fprintd and slock-ly are unchanged and existing fingers keep working.
- Differences: T480-only (06cb:009a / type 0x199); pairing identity pinned in /etc/validity-rs.conf
  (coreboot SMBIOS `T480 123456789`, stock BIOS `20L6S4VC00 PF0XXXXX` as fallback) so a firmware
  swap no longer breaks the pairing; recovers a wedged sensor (USB reset / reboot) instead of
  needing a reboot; Suspend/shutdown end a running verify with `verify-disconnected` so slock-ly
  re-arms at once; ~0.65 s bring-up, ~0.75 s resume; 5 MB RSS, 0 CPU idle.
- Verified 2026-09-29 without fingers: tests/nofinger-battery.sh 17/17 (100 cancel cycles,
  10 suspend aborts, 10 restarts, SIGKILL recovery, open-fprintd restart, 0 sensor disconnects);
  installed service: 30 cancel cycles, 3 suspend aborts, 0 disconnects. Finger tests pending
  (fprintd-verify, slock-ly unlock, suspend/resume unlock, tap-during-cancel).
- Install incident: `pacman -U` fired the `s6-rc-db-update` hook → s6-rc-update "Broken pipe",
  /run/service 73 → 4. Repaired per the 2026-09-28 recipe (relink servicedirs, `down` where
  s6-svok fails, `s6-svscanctl -a`); no service restarted. Switched with `s6-rc -d/-u change`,
  persisted with `s6 set disable python3-validity`, `s6 set enable validity-rs`, `s6 set commit`
  (no `s6 live install`). /run/service = 74.
- Config snapshots: corpus/configs/etc/validity-rs.conf, corpus/configs/etc/s6/sv/validity-rs/.
- Graph/wiki (graphify-out) are stale for the fingerprint stack until regenerated.
