# syswork — design rationale

Date: 2026-09-19. Status: implemented (`syswork/syswork`, v0.1.0).
User doc: `syswork/README.md`. Mirror for AST extraction:
`corpus/sources/syswork.sh`.

## Why overlayfs + git on ext4

The T480 runs ext4 with no snapshot-capable layer. Safe system
experimentation needed staging + revert without reinstalling the
storage stack.

- **Rejected: btrfs migration** — converting the live root filesystem
  for tooling alone risks the running system to gain snapshots; ext4
  + overlayfs already gives copy-on-write staging with zero migration.
- **Rejected: OSTree / image-based updates** — full deployment model
  (ostree remotes, bootloader entries, reboot-to-apply) is heavier
  than needed for config-level experiments on a single hand-tended
  laptop; overlayfs gives per-file staging without a new boot model.

overlayfs + git is the minimal composition: the kernel already
provides the staging primitive, git already versions text. The CLI
just wires them together with guardrails.

## Overlay mechanics

- One tree = one upper + work dir under `/var/lib/syswork/<name>/`
  (700, root-owned), merged at `/srv/work/<name>`.
- Mount: `mount -t overlay overlay -o
  lowerdir=/,upperdir=$U,workdir=$W,index=off $M`.
- `lowerdir=/` means the whole live root is visible inside the merged
  view; writes land in the upper (changed files only — disk cost is
  proportional to the experiment, mount cost ~ms).
- Changed-path enumeration excludes pseudo-fs and git internals
  (`proc sys dev run tmp mnt media swapfile`, `.git`, plus the
  syswork dirs themselves) so diffs show only real edits.
- Apply = `rsync -aAXH` (no `--delete`, no perm/owner clobbering)
  from upper to `/`, then per-repo git commits. Deletions are
  deliberately not propagated — removing a live file needs explicit
  handling, not an rsync side effect.
- Binaries (`.rom .bin .img .iso .fw`, kernel images) are
  manifest-only: sha256 goes to the audit log, blobs never enter git
  (enforced by `.gitignore` from `init-repos` + a `--full`-style
  binary regex at commit time).

## Revert ladder

Three rungs, escalating cost (see README for commands):

1. `drop` — unmount + delete upper. Millisecond revert, works only
   *before* apply (live was never touched).
2. `git checkout` — per-repo commits on every apply (`/etc`,
   `/usr/local`, `~/.config`, `~/bin`). Second-level revert for
   already-applied changes, targeted to single files.
3. timeshift — full-system restore, the nuclear option for broken
   boot or changes outside git's reach (binaries, MBR, bulk damage).

The ladder works because each rung covers the previous rung's blind
spot: drop can't undo applies, git can't restore unversioned bytes,
timeshift doesn't care about any of that.

## Boot budget

~0 by construction: no daemons, no timers, no s6 services. Worktrees
are oneshot mounts (re-merge via `mount-all` is manual after reboot).
Git repos are passive data. Runtime footprint of the tool itself is
one bash process per invocation; the only persistent costs are disk
(uppers, merged views, audit log) and whatever experiments you leave
mounted — `syswork list` shows them, `drop` reclaims them.

## Built-in guards (why they're in the tool, not docs)

- **Sensitive gate** — boot images, shadow/sudoers, ssh host keys,
  and `root/.ssh/` abort apply unless `--allow-sensitive` is passed.
  Rationale: these paths either brick boot when wrong or leak trust
  material when committed carelessly; the flag forces a conscious
  second decision at the exact moment of danger.
- **Thermal single-writer warning** — apply warns if the changeset
  touches `thinkfan.yaml`/`throttled.conf` while thermald-t480 is
  alive (per `notes/thermal.md`). A warning, not a block: switching
  thermal owners is legitimate, just never accidental.
- **No `--delete` on rsync** — see Overlay mechanics above.
- **Append-only audit log** — `$AUDIT_LOG` gets every mutation with
  timestamp + invoking user; best-effort write (never fails the op).

## Open questions / non-goals

- Upstream configs drift: apply commits capture *what* landed but not
  *why* — substantive changes still need a notes entry per AGENT.md.
- No multi-machine story (single T480; NAS mirror is just a copy).
- Shadow binds don't survive reboot (bind mounts, not fstab) — by
  design, so a bad single-file test can't brick the next boot.
