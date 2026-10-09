# Home

| Here | Goes to |
|---|---|
| `.xinitrc`, `.zprofile`, `.zshrc`, ... | `~/` |
| `config/` | `~/.config/` |
| `local-bin/` | `~/.local/bin/` |
| `local-share/` | `~/.local/share/` |

The files in this directory start with a dot: use `ls -a`.

- tty1 logs in automatically, `.zprofile` runs `startx`, `.xinitrc` starts the lock
  screen first and then the session.
- `~/.config/chadwm` is a link to the chadwm source tree (`../src/chadwm`); its
  `scripts/run.sh` starts picom, the bar and the window manager.
- zsh plugins are not included: `config/zsh/plugins/PLUGINS.txt` lists what to clone.
- The Bitwarden desktop override uses `igpu-run` from `src/gpu-power/` to select
  Intel/Mesa on its next launch. See `docs/wiki/gpu-routing.md` for tests and rollback.
