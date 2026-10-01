# syswork NAS mirrors + timeshift + audit log

Host: Artix T480. NAS: `192.168.0.23:/my-zfs`, mounted at `/mnt/my-zfs`
(fstab: `nfs noauto,nofail,soft,timeo=10,retrans=2,_netdev` — on demand only).
All privileged commands run as `ssh -o BatchMode=yes root@localhost '<cmd>'`. NEVER sudo.

## 1. Mirror push procedure (exact steps)

Bare repos already created on NAS: `/mnt/my-zfs/syswork-mirrors/etc.git`,
`/mnt/my-zfs/syswork-mirrors/usr-local.git`.

```sh
# mount on demand (fails gracefully if NAS is down — abort, do NOT force)
ssh -o BatchMode=yes root@localhost 'mount /mnt/my-zfs'
ssh -o BatchMode=yes root@localhost 'mountpoint -q /mnt/my-zfs || echo NAS_DOWN_ABORT'

# check branch names first (init-repos may use master OR main)
ssh -o BatchMode=yes root@localhost 'git -C /etc branch --show-current; git -C /usr/local branch --show-current'

# push (substitute actual branch name for <br>)
ssh -o BatchMode=yes root@localhost 'git -C /etc push /mnt/my-zfs/syswork-mirrors/etc.git <br>'
ssh -o BatchMode=yes root@localhost 'git -C /usr/local push /mnt/my-zfs/syswork-mirrors/usr-local.git <br>'

# verify
ssh -o BatchMode=yes root@localhost 'git ls-remote /mnt/my-zfs/syswork-mirrors/etc.git; git ls-remote /mnt/my-zfs/syswork-mirrors/usr-local.git'

# ALWAYS unmount afterwards (run from a cwd outside /mnt/my-zfs so it isn't busy;
# 2026-09-19: plain umount sufficed, lazy fallback not needed)
ssh -o BatchMode=yes root@localhost 'umount /mnt/my-zfs || umount -l /mnt/my-zfs'
ssh -o BatchMode=yes root@localhost 'mountpoint -q /mnt/my-zfs && echo STILL_MOUNTED || echo UNMOUNTED'
```

To re-create a bare mirror from scratch (if ever deleted):
`git init --bare /mnt/my-zfs/syswork-mirrors/<name>.git` (while mounted).

## 2. Proposed cron line (NOT installed — for user approval)

```cron
# Mirror syswork repos to NAS every 6h (mount -> push -> unmount). Mount failure aborts silently.
0 */6 * * * root mountpoint -q /mnt/my-zfs || mount /mnt/my-zfs; mountpoint -q /mnt/my-zfs && { git -C /etc push /mnt/my-zfs/syswork-mirrors/etc.git $(git -C /etc branch --show-current); git -C /usr/local push /mnt/my-zfs/syswork-mirrors/usr-local.git $(git -C /usr/local branch --show-current); umount /mnt/my-zfs; }
```

## 3. timeshift (NOT installed — absent on 2026-09-19)

To install it (pacman asks before it installs anything):
```sh
sudo pacman -S timeshift && sudo timeshift --create --comments "baseline" --tags D
```
Setup notes: use **rsync mode**; in the profile EXCLUDE `/srv/work` and
`/var/lib/syswork` (snapshot balloon risk); disable all boot-time snapshot
jobs (no scheduled boot snapshots, no boot-time actions).

## 4. Audit log rotation (`/var/log/syswork.log`, root-owned 640, `chattr +a`)

Append-only means even root cannot overwrite/truncate without clearing the flag.
Rotate like this:
```sh
ssh -o BatchMode=yes root@localhost 'chattr -a /var/log/syswork.log && mv /var/log/syswork.log /var/log/syswork.log.1 && touch /var/log/syswork.log && chown root:root /var/log/syswork.log && chmod 640 /var/log/syswork.log && chattr +a /var/log/syswork.log && lsattr /var/log/syswork.log'
```
Keep the window between `-a` and `+a` as short as possible. gzip old rotations.
