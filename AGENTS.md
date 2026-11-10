# Notes for agents working in this repository

This is the rebuild kit of one ThinkPad T480. It is a copy of what runs on that
laptop: firmware patches, a kernel package, `/etc`, `/usr/local`, dotfiles and
the programs written for it. It is public.

## Before you finish any change

    make check

It runs a shell syntax pass over every tracked script and the three test
suites (`tools/re-audit/tests`, `src/gpu-power/tests`, `src/thermald-t480/tests`).
It needs `python3`, `cc`, `patch` and, for the ACPI tests, `iasl` and
`acpiexec`. It never touches hardware and needs no root. Three skipped tests
are normal (they want `T480_COREBOOT_ROOT`, a coreboot source tree).

## Rules

- Nothing that identifies the machine or its owner goes in: no MAC address,
  serial number, partition UUID, IP address, Wi-Fi or VPN configuration, e-mail
  address. Placeholders are quoted strings or `${VAR:?message}`, never bare
  `<...>` in a shell script.
- Never run the scripts in `firmware/tools/`, `system/etc/elogind/system-sleep/`,
  `system/etc/local.d/` or `kernel/early-init/` to try them out. They flash
  firmware, suspend the machine or repartition disks. Read them.
- `system/etc/` and `system/usr-local/` mirror live files. A change here does
  nothing until the owner deploys it on the laptop; say so in your summary.
- Files kept in two places must stay identical: `src/gpu-power/prime-run` and
  `system/usr-local/bin/prime-run`.
- Commit messages are one plain sentence, no prefix, no trailers.
