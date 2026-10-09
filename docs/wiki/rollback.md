# Restoration and experiment boundaries

Original programs/configs are preserved on the laptop in
`/root/myt480-audit-20261009` (root-only directory). Keep that directory until the
changes have been exercised in daily use. It contains the original thermal/GPU
programs, original `prime-run`, and original configs. The initial GPU mode was
automatic; there was no manual-mode marker to restore.

The checksum-checked [rollback script](../../tools/re-audit/rollback.sh) has been
copied into the same directory. Both its preflight and full `all` restoration
were exercised. All three original executable hashes and the original GPU
config hash matched afterward. The improvements were then reapplied, and CUDA,
PRIME rendering, service state, and final file hashes were checked again.

During reapplication, Bitwarden opened an NVIDIA device. One reload attempt was
refused because the GPU was in use; error cleanup/reapplication restored the new
programs and coarse-mode driver. The final GPU was active/D0 with that application
holding device handles. The application was left running. This demonstrates why
restoration requires GPU clients to be closed, and why configuration and active
parameters must be checked separately.

As root on the T480:

```sh
# Verify all backups/endpoints first; performs no changes.
sh /root/myt480-audit-20261009/rollback.sh all --check

# Restore one subsystem or both. Close GPU applications before gpu/all.
sh /root/myt480-audit-20261009/rollback.sh thermal
sh /root/myt480-audit-20261009/rollback.sh gpu
# Or:
sh /root/myt480-audit-20261009/rollback.sh all
```

Thermal restoration stops the s6 service, atomically replaces its executable,
restores its config, and starts it again. GPU restoration refuses to start with
observed GPU clients, unloads the driver, atomically restores both programs,
restores the config and original mode, then applies the policy. A new client
appearing during restoration can still make unload fail; rerun after closing it.

For a policy-only GPU rollback, keeping the wrapper fixes:

```sh
sed -i 's/^rtd3 .*/rtd3 0/' /etc/gpu-power.conf
gpu-power off
gpu-power auto
```

Changing the config alone does not change an already loaded NVIDIA module.
Check `gpu-power status` afterward. The original AC setting shows parameter
`0x03`, active/D0; the audited idle state shows parameter `0x01`, suspended/D3cold.

## Original hashes

| Backup | SHA256 |
|---|---|
| `thermald-t480` | `bb79265098040ad54f588e742d46dbb23e55f06407e559db77296d80659e87f0` |
| `thermald.conf` | `960b36ec57354117c2449e67425420976714ac13595b25005bf9f1f326c0205a` |
| `gpu-power` | `359b63d520fadc2b99977a39b96d9ada323934d418801c00bfcd0a3aa8e710f4` |
| `prime-run` | `c34ab783d381d6e4269650da1db4883bcd1a571ba163cd6d08ce43125da1f488` |
| `gpu-power.conf` | `4cacdfbfe7853a60033b2d2fff2b5a32eb7bb37d8fd9e7ff68426826034215d4` |

Final deployed checksums: thermal
`c64159a5f21100e3f55fd8ca8c228c5aaee8d15bc615c76f451b52da0f463f61`,
GPU helper `57b9481efaab2c5ee72f5a678b2b9dd026b26740e9cb43eb033a86265c1aa192`,
wrapper `65d1e3948ac8c22645af9d7037d729832ab9908ce96d7e80aa6bf6b799e01666`,
GPU config `d76537387b7e79d7a358b647cc01780e084dbc988512173e721d80cd6a2edaff`.

## Repository and evidence

Changes are isolated on local branch `re-audit-20261009`; the original local
`main` branch was not rewritten. The branch began at the fetched current remote
baseline `d89f79d`. No commits were pushed and no firmware was flashed by this audit.

Raw reverse-engineering output and selected validation logs are kept privately
on the workstation under `/home/btw/test/rea/work/audit-20261009`. Proprietary DLLs,
full firmware images, biometric data, laptop identities, and keys are not added
to the public repository. The wiki contains narrow derived findings and hashes.

To inspect the branch without deploying it, read the source diff and run the
hardware-independent tests. The read-only collector is safe to repeat. The
RTD3 test temporarily reloads NVIDIA and therefore belongs on this T480, on AC,
with GPU clients closed. It restores its saved policy automatically on exit.
