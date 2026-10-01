# syswork — whole-root worktrees on ext4

`syswork` stages system changes in an overlayfs worktree instead of
editing the live root in place. Edit under `/srv/work/<name>/`,
review the changeset, then apply (rsync to live + git commit) or
drop (instant revert). Version 0.1.0.

## Why

This T480 runs ext4 — no btrfs snapshots, no OSTree deployments, and
migrating filesystems for safety tooling alone isn't worth it.
overlayfs gives snapshot-like behavior with kernel primitives already
present: mount cost is milliseconds, disk cost is changed files only,
and the live system is untouched until you say otherwise.

Caveats up front: running daemons keep reading LIVE paths — the merged
view at `/srv/work/<name>` is a staging area, not a container.
Pseudo-filesystems (`/proc /sys /dev /run`) inside the merged view are
empty underlying dirs; read sensors and service state from live paths.
After a reboot, uppers persist on disk but need `syswork mount-all`
to be re-merged.

## Command examples — a wifi-test flow

```bash
sudo syswork new wifi-test
# edit /srv/work/wifi-test/etc/NetworkManager/... (live untouched)

syswork diff wifi-test            # file list of the changeset
syswork diff wifi-test --full     # unified diffs, review before apply

# live-test a single file without applying the whole tree:
sudo syswork shadow wifi-test /etc/NetworkManager/system-connections/example.nmconnection
nmcli connection reload           # exercise the change
sudo syswork unshadow /etc/NetworkManager/system-connections/example.nmconnection

sudo syswork apply wifi-test --yes --drop   # commit to live + git, then drop tree
```

More commands: `list` (all trees: mounted? size, changed files),
`status <name>` (mount state + change stats), `drop <name> --yes`
(instant revert), `mount <name>` / `mount-all` (re-merge after reboot),
`shadows` (active single-file binds), `init-repos` (git-init /etc +
/usr/local with binary excludes), `help`.

Single-file live tests via `shadow` also work for quick thermal-curve
experiments — but remember the thermal single-writer rule: parked
thinkfan/throttled configs won't take effect while thermald-t480 runs
(syswork warns you; switching owners is a deliberate manual step).

Sensitive paths (boot images, `etc/shadow`, sudoers, ssh host keys,
`root/.ssh/`) refuse to apply unless you pass `--allow-sensitive`.
Binaries (`.rom .bin .img .iso .fw`, kernel images) are never blobbed
into git — apply logs a sha256 in the audit log instead (manifest-only).

## First-time setup

```bash
sudo syswork init-repos     # git-init /etc + /usr/local, binary excludes, ssh-signing
syswork list                # should print "(no worktrees)"
sudo syswork new smoke-test # verify overlay mounts on this kernel
syswork diff smoke-test     # expect "--- 0 changed path(s)"
sudo syswork drop smoke-test --yes
```

Requires: root (via `sudo -n`), kernel overlayfs support, `git`,
`rsync`. Uppers live under `/var/lib/syswork/`, merged views under
`/srv/work/` — both created on demand. Nothing runs until you create
your first tree.

## Revert ladder (cheap → nuclear)

1. **`drop` (milliseconds)** — `sudo syswork drop <name> --yes`
   unmounts + deletes the upper. Unapplied experiment gone, live never
   touched. `unshadow` is the same idea for single-file tests.
2. **`git checkout` (seconds, targeted)** — every apply commits per
   repo (`/etc`, `/usr/local`, `~/.config`, `~/bin`, where `.git`
   exists). Revert one file: `git -C /etc log --oneline`,
   `git -C /etc checkout <commit> -- <path>`.
3. **timeshift (minutes, nuclear)** — full-system restore for when the
   change already broke boot or something outside git's reach.

## Boot impact (~0)

- Worktrees are **oneshot mounts** — nothing runs at boot unless you
  re-merge (`mount-all` is manual, e.g. from rc.local or by hand).
- Git repos under `/etc` and `/usr/local` are **passive data** —
  no hooks, no daemons, no periodic jobs.
- **No daemons, no timers, no s6 services.** The only footprint is
  disk: uppers under `/var/lib/syswork/`, merged views under
  `/srv/work/`, audit log at `/var/log/syswork.log`.

## Security model

- **Name validation** — tree names match `[a-z0-9_-]`, max 32 chars;
  live-paths for shadow must be absolute and exist on both sides.
- **Uppers are 700 root-owned** under `/var/lib/syswork/` — unmerged
  experiment contents aren't world-readable.
- **Sensitive gate** — footgun/secrets-adjacent paths need
  `--allow-sensitive` on apply (see above); without it, apply aborts
  and lists the offending paths.
- **ssh-signed commits** — `init-repos` configures `gpg.format ssh`
  with the existing root ed25519 key, so apply commits are signed
  best-effort (falls back to unsigned rather than failing).
- **Append-only audit log** — every `new/mount/apply/drop/shadow`
  appends `timestamp user=... action` to `/var/log/syswork.log`.
- **NAS mirror pointer** — git repos + audit log are the backup
  contract; mirror `/etc/.git`, `/usr/local/.git`, and
  `/var/log/syswork.log` to the NAS on your regular schedule so the
  revert ladder survives disk loss, not just bad edits.
