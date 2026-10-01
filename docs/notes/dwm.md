# dwm setup — ThinkPad T480, Artix Linux (s6), dwm-titus fork

> History: the dwm-titus desktop the laptop ran until 2026-09-20. Since then the desktop
> is chadwm (`src/chadwm/`; see the section of 2026-09-20 below).

## Overview

- Heavily patched dwm fork from ChrisTitusTech/dwm-titus (upstream is Fedora-only;
  this host is Artix, PID1 = `s6-svscan`, so systemd-user assumptions are partly inert).
- `config.mk` VERSION = `0.7.0`, `PREFIX=/usr/local`. MODKEY = Super.
- Compile-time `config.h` is minimal (appearance/tags/layouts only); keys, themes,
  window rules are runtime TOML with inotify hot-reload (`kill -USR1 $(pidof dwm)`).
- Live session is started with `startx` via `/home/btw/.xinitrc` (thin boot env +
  `exec /usr/local/bin/dwm`). SDDM entry exists but is not the live path.
- Shell/bar duties are delegated to managed Quickshell
  (`quickshell --path /home/btw/.config/quickshell/shell.qml --no-duplicate`),
  plus small `/usr/local/bin/dwm-*` glue scripts.
- Repo checkout `/home/btw/build/dwm-scratch/` has one uncommitted change (see below);
  installed `/usr/local/bin/dwm` does NOT match a fresh repo build.

## Sources & Binaries (path → role)

- `/home/btw/build/dwm-scratch/` — source checkout. `git log` tip `68a0d1f`,
  `git status --short`: `M dwm.c` only (+108/-6, scratch workspace patch).
- `/home/btw/build/dwm-scratch/dwm.c` (+`drw.c/h`, `util.c/h`, `tomlparser.c/h`) — WM source.
- `/home/btw/build/dwm-scratch/config.h` == `config.def.h` (identical) — appearance,
  tags, layouts, MODKEY. No key/rule tables (moved to TOML).
- `/home/btw/build/dwm-scratch/config.mk` — build config: pkg-config modules
  `x11 xft xinerama xrender imlib2 x11-xcb xcb xcb-res fontconfig freetype2`,
  `-O2` default / `-O3 -march=native -flto=auto` via `make native`.
- `/home/btw/build/dwm-scratch/SPEC.md` — product spec (source of truth: 5.1 WM,
  5.2 runtime config, 5.3 session startup, 5.4 launcher, 5.10 settings, images).
- `/home/btw/build/dwm-scratch/AGENTS.md` — engineering rules (Fedora contract,
  Quickshell event-driven/idle rules, validation via `scripts/run-tests`).
- `/home/btw/build/dwm-scratch/TASKS.md` — active phase only (currently desktop-updates
  PR #318 work, all boxes checked, no merge).
- `/home/btw/build/dwm-scratch/README.md` — v0.7.0 Fedora ISO + `./install.sh` docs.
- `/home/btw/build/dwm-scratch/config/` — default TOMLs + app configs
  (`hotkeys.toml`, `themes.toml`, `window-rules.toml`, `alacritty/`, `kitty/`,
  `quickshell/`, `starship/`, `systemd/`, `xdg-desktop-portal/`, `Thunar/`).
- `/home/btw/build/dwm-scratch/scripts/` — session + helpers mirrored into
  `/usr/local/bin/` (`autostart.sh`, `dwm-session-launch`, `dwm-status`,
  `dwm-lock`, `dwm-lock-watch`, `dwm-settings-*`, `dwm-quickshell-*`, etc.).
  Note: repo `scripts/.xinitrc` is only `dbus-run-session -- sh -c 'exec dwm'` —
  NOT the live `~/.xinitrc`.
- `/home/btw/.config/dwm-titus/` — live runtime config (see below).
- `/usr/local/bin/dwm` (140864 B, md5 `ba793d45…`, 2026-09-15 22:04) — live binary.
  Matches `/usr/local/bin/dwm.bak-pre-scratch-20260915`, DIFFERS from repo-built
  `/home/btw/build/dwm-scratch/dwm` (141000 B, md5 `3a8a0011…`, 22:00). Installed
  binary predates the uncommitted scratch patch.
- `/usr/local/bin/dwm-session-launch` — identical to repo copy (md5 `ce9a07ff…`).
  `spawn()` wrapper: validates/parses `theme-env.sh` under flock, exports
  QT/XCURSOR env, then `exec "$@"`. Every hotkey `spawn` goes through it
  (resolved via `/proc/self/exe` sibling).
- `/usr/local/bin/dwm-keybinds` (266 B) — `quickshell ipc … call controlcenter openKeybinds`.
- `/usr/local/bin/dwm-status` (9.9 KB bash) — minimal root-name publisher
  (`xsetroot -name "BAT … | VOL …"`), pactl + udevadm event-driven, FIFO +
  restart limits, per-DISPLAY identity file.
- `/usr/local/bin/dwm-lock` (4.0 KB) — locker cascade (see Session). Has
  `.bak-pre-blacklock-20260916` (3.1 KB); live adds brightness logic.
- `/usr/local/bin/dwm-lock-watch` (1.5 KB) — logind `Lock` signal → `dwm-lock`
  (`DWM_LOCK_NO_LOGINCTL=1`).
- `/usr/local/bin/dwm-polkit` (1021 B) — first-available polkit agent launcher.
- `/usr/local/bin/dwm-titus-session` (637 B) — SDDM-style session script (NOT live path).
- Helpers: `slock` + `slock-ly` (both setuid root), `bright` at
  `/home/btw/.local/bin/bright` (brightnessctl wrapper, STEP=10, DEV=acpi_video0,
  state `~/.local/state/brightness-pct`), `super-tap-c` (audio tap).

## Keybindings model (hotkeys.toml structure)

- Live file: `/home/btw/.config/dwm-titus/hotkeys.toml` (repo default at
  `/home/btw/build/dwm-scratch/config/hotkeys.toml` — DIFFER).
- Sections: `[vars]` (`terminal="st"`, `webapp="webapp-launch"`), `keys=[…]`,
  `tag_keys=[…]`, `buttons=[…]`. Repo header documents every `func`; live file
  replaced it with a 4-line personal comment (SUPER+t terminal, SUPER+f fullscreen,
  SUPER+a launcher, SUPER+k kbd-lang, CAPS toggles group).
- One binding per line:
  `{ mod="SUPER SHIFT", key="x", desc="…", func="spawn", exec=[…] | cmd="…" }`
  mods: `SUPER SHIFT CTRL ALT` space-separated, `""` = none. Args: `i=±1`,
  `f=±0.05/±0.25/0.00`, `ui=-1`, `layout_idx=0/1/2`.
- Func vocabulary (from `dwm.c` string table): `spawn killclient quit focusstack
  movestack incnmaster setmfact setcfact zoom view toggleview tag toggletag
  togglebar togglefloating togglescratch togglescratchtags fullscreen
  togglefakefullscreen setlayout focusmon tagmon`.
- Personal remaps vs repo default: `SUPER+t` → `st` (was `$terminal`=alacritty),
  `SUPER+x` → alacritty, `SUPER+f` → `fullscreen`, `SUPER+w/space` → togglefloating,
  `SUPER+l` → `loginctl lock-session`, `SUPER+k` → `kbd-lang.sh toggle` (us,ara),
  `SUPER+s/grave` → `togglescratch`, brightness → `bright up/down` (was
  `brightnessctl set 10%±`), `SUPER+SHIFT+w` → `dwm-settings-wallpaper randomize`,
  launchers via `quickshell ipc … call launcher toggle`, `dwm-terminal`,
  `webapp-launch`, `firefox`, `pcmanfm-qt`; media via `playerctl`/`amixer`/`pactl`;
  screenshots `dwm-screenshot screen/gui/clip/full` (all modes put a PNG on the clipboard; screen/gui/full also save a JPEG, 2026-09-29); OCR `maim -s | tesseract …
  eng+ara | xclip`; placeholders (`notify-send Dictation/GameMode`).
- `tag_keys`: 9 entries `{key="1"…"9", tag=0…8}`; each auto-generates 4 bindings
  (view / view+ / send / show-window-on).
- `buttons`: ClkTagBar SUPER+LMB `tag`, SUPER+RMB `toggletag`, bare LMB `view`,
  RMB `toggleview`; ClkClientWin SUPER+LMB `moveorplace i=2`, SUPER+RMB `resizemouse`.
- Reload: inotify watches on config dir + `kill -USR1`; invalid TOML keeps last
  valid state. Backups show recent edits: `bak-pre-bright-20260916`
  (brightnessctl→bright, alacritty→st), `bak-pre-grave-20260915` /
  `bak-pre-scratch-20260915` (scratchpad `toggleview ui=256` → C `togglescratch`).

## Theming (themes.toml + theme-env.sh)

- Live: `/home/btw/.config/dwm-titus/themes.toml`; `[active] theme="monochrome"`
  (repo default `"nord"`; `themes.toml.pre-mono` pins nord). `[appearance]
  borderpx=1`. Only other repo-vs-live theme diff: monochrome `term_color4`
  `#6699CC` → `#CC3333`.
- 15 `[theme.NAME]` sections, each: `normfg/bg/border`, `sel fg/bg/border`,
  `term_bg/fg/cursor`, `term_color0…15`, `dark_mode`, optional `gtk_theme`.
  Dark: nord dracula gruvbox catppuccin tokyonight onedark solarized rosepine
  everforest monochrome. Light: catppuccin-latte gruvbox-light solarized-light
  rosepine-dawn tokyonight-day. Nord GTK = `Nordic`.
- Active monochrome values: norm `#AAAAAA/#222222/#333333`,
  sel `#FFFFFF/#333333/#FFFFFF`, term bg `#1A1A1A` fg `#CCCCCC` cursor `#FFFFFF`.
- `/home/btw/.config/dwm-titus/theme-env.sh` (auto-generated, do-not-edit):
  `QT_QPA_PLATFORMTHEME=gtk3`, `XCURSOR_THEME=Capitaine-Cursors-White`,
  `XCURSOR_SIZE=32`. Sourced by `~/.xinitrc`, `autostart.sh`, `dwm-titus-session`;
  re-validated per-spawn by `dwm-session-launch` (flock on
  `$XDG_RUNTIME_DIR/dwm-theme-apply.lock`, ≤4096 B, uid/link/size checks).
- Wallpaper: `/home/btw/.config/dwm-titus/wallpaper.conf` →
  `version=1 mode=selection path=/home/btw/Pictures/wallpapers/wallpaper.png fit=fill`.
  `/home/btw/Pictures/wallpapers/` holds `wallpaper.png` + `dyj2v1s7aoz01 (1).png` (same 1224500 B)
  + `t480-exploded.png`.

## Window rules

- Live `/home/btw/.config/dwm-titus/window-rules.toml` == repo default (SAME;
  `.bak-pre-scratch-20260915` also identical).
- Fields: `class` (res_class) `instance` (res_name) `title` `monitor` `tags` 1–9
  (0/omit = current) `isfloating` `isterminal` `alwaysontop` `noswallow` (+`float`
  legacy on RAIL). All matching rules apply in order.
- Terminals (`isterminal=1` → swallowing): `Dwmterm St kitty Alacritty
  warp-terminal Terminator`. `RAIL float=1`. `Event Tester noswallow=1`.
- Quickshell surfaces floating+ontop by title: `dwm launcher`, `dwm menu`,
  `dwm network password`, `dwm control center utility`, `dwm system health`,
  `dwm settings`, `dwm notification history`.
- Tag pinning: only active is `{class="PlazmicLegends", tags=5, monitor=1,
  noswallow=1}`; firefox/discord/Steam/Gimp/pavucontrol entries commented out.
- Find classes with `xprop | grep -E "WM_CLASS|WM_NAME"`.

## Session startup order

- Live (`startx` → `/home/btw/.xinitrc`): `slock-ly &` boot cover (`SLOCK_USER=btw`)
  → env (`NO_AT_BRIDGE=1`, `GTK_A11Y=none`, `QT_QPA_PLATFORMTHEME=gtk3`,
  `XCURSOR_THEME/SIZE` + source `theme-env.sh`) → `feh --bg-fill
  /home/btw/Pictures/wallpapers/wallpaper.png` → `xset r rate 250 50` → `bright restore` →
  `dbus-launch` if needed → `pulseaudio --start` → `super-tap-c` (inherits
  DISPLAY/XAUTHORITY, log `~/.local/share/super-tap.log`) → `exec /usr/local/bin/dwm`.
- Inside dwm: `runautostart()` runs `scripts/autostart.sh` resolved from
  `~/.local/share/dwm` else `~/.dwm` — NEITHER EXISTS live (only
  `~/.local/share/dwm-titus/` + `~/.local/share/dwm-titus/scripts/` do).
- Repo `scripts/autostart.sh` (if it ran) Phase 1 blocking: `resume_theme_preview`
  → `apply_power_settings` → `dwm-settings-input apply-saved` + `watch-apply` →
  source `theme-env.sh` → `dwm-xsettings session-apply` → set
  `XDG_CURRENT_DESKTOP+=dwm:X-DWM`, `QT_QPA_PLATFORM=xcb`, unset WAYLAND →
  systemctl/dbus env import → quickshell version-check + `start_managed_quickshell`
  + wait tray → start `wm-graphical-session.service`. Phase 2 background: wallpaper
  `session-apply` (fallback `feh --randomize ~/Pictures/backgrounds`) → `picom start`
  → `dwm-status` (display-locked singleton) → `dwm-lock-watch` → first polkit agent
  → `dex` fallback → `apply_power_settings` again.
- `dwm-lock` cascade: brighten to 100% (raw, unsaved) → `light-locker-command
  --lock` (transient locker) → `xdg-screensaver lock` → `loginctl lock-session`
  (skipped under watch via `DWM_LOCK_NO_LOGINCTL=1`) → mate/xfce/cinnamon/gnome
  saver → `dwm-lock-bright slock` → `slock-ly` → `i3lock/slock/xlock`; `bright
  restore` after EVERY success path. `SUPER+l` triggers `loginctl lock-session`,
  caught by `dwm-lock-watch`.
- `/usr/share/xsessions/dwm.desktop`: `Exec=/usr/local/bin/dwm` (installed,
  concrete). Repo `dwm.desktop` still has `@PREFIX@` template.
- `dwm-titus-session` (sddm-flavored, unused live): same env, wallpaper
  `/home/btw/Pictures/t480-exploded.png`, hardcoded `DISPLAY=:0`, `super-tap`
  (not `-c`).

## Xresources

- `/home/btw/.Xresources`: urxvt nord + Meslo (`URxvt.font: MesloLGS Nerd Font
  Mono:size=11,JetBrainsMono Nerd Font, Noto Color Emoji`; `saveLines: 10000`;
  `scrollBar: false`; fg `#e5e9f0` bg `#2e3440` cursor `#88c0d0`; color0–15 nord).
- `/home/btw/.config/dwm-titus/cursor.Xresources`: `Xcursor.theme:
  Capitaine-Cursors-White`, `Xcursor.size: 32` (mirrors theme-env).
- `/home/btw/.config/dwm-titus/xsettingsd.conf`: auto-generated, body empty.
- Misc live state: `kbd-lang` contains `ara,us`; `notification-settings.json`
  `{"version":1,"doNotDisturb":false,"popupTimeoutMs":6000}`; `wallpaper.conf`
  mode 600.

## Notable patches/customizations vs stock dwm

- Runtime TOML engine (`tomlparser.c`, inotify + SIGUSR1): keys, buttons,
  tag_keys, themes, window/title rules all hot-reload; `config.h` keeps only
  appearance (`refresh_rate=60`, `enable_noborder=1`, `cursorwarp=1`, `snap=32`,
  `swallowfloating=0`, `showbar/topbar=1`, `ICONSIZE=17`, `SHOWWINICON=1`,
  MesloLGS+NotoColorEmoji fonts), tags `1–9` (+`ptagf/etagf/lcaselbl`), layouts
  `[]=` tile / `><>` float / `[M]` monocle, `mfact=0.55 nmaster=1
  resizehints=1 lockfullscreen=1`, `STATUSBAR "dwmblocks"` (vestigial).
- Uncommitted scratch patch (live binary LACKS it): `togglescratch` /
  `togglescratchtags` special workspace — `scratchtag = 1u << 9` past last tag,
  `ISVISIBLE = TAGVISIBLE || SCRATCHVISIBLE`.
- Stock-plus set: pertag (per-tag layout/state), `movestack`, `setcfact`,
  `focusmon/tagmon`, real `fullscreen` + `togglefakefullscreen`, cursor warp,
  noborder toggle, swallowing (`isterminal/noswallow`), `_NET_WM_ICON` title icons,
  title-match rules (`applytitlerules`), EWMH, Xinerama, per-monitor tag masks,
  `alwaysontop`, autostart/autostop hooks, SIGCHLD-safe spawn via
  `dwm-session-launch`, `moveorplace`.
- Bar/status outsourced: Quickshell panel draws the bar (`bh=0`; its window is
  adopted as `barwin` — 1920x30 at +0+0). `dwm-status` is meant to publish
  `BAT % | VOL %` via `xsetroot` for it, but `xsetroot` is NOT installed on this
  box (`xprop -root WM_NAME` = "not found", checked 2026-09-20), so the
  root-name channel has never worked; the panel's own widgets use UPower/nmcli/
  pactl directly. A native-suckless-bar attempt (slstatus + stalonetray,
  2026-09-19) was fully reverted 2026-09-20 — see dated section below.

## 2026-09-20 — native-bar (suckless) migration reverted

The bar work was reverted in full, with an X restart.

What the bar work had landed (2026-09-19 19:45 → 2026-09-20 02:30, syswork
worktrees `dwmbar`, `dwmbar2`, `slbar`; plan/surveys in
`suckless/notes/{bar-plan,launcher-plan,st-fix,sources-survey}.md`):
`/usr/local/bin` bar builds of `dwm`/`st`/`dmenu`/`dmenu_run` plus `slstatus`,
`nsxiv`, `super-tap-suckless.py`, `dmenu-{apps,clip,power}.sh`; `~/.xinitrc`
mode switch on `DWM_USE_QUICKSHELL` + direct quickshell launch; `autostart.sh`
quickshell gated off and slstatus+stalonetray block added; `hotkeys.toml`
launcher/cheatsheet/clipboard/power-menu rebound from quickshell IPC to dmenu
scripts + `loginctl reboot|suspend` + `kill -USR1` reload; `~/.local/bin/
slstatus-widgets.sh` added; `stalonetray` package installed.

Revert, verified live 2026-09-20 08:55 (+0300):
- `/usr/local`: `git checkout 658f537 -- bin/{dwm,st,dmenu,dmenu_run}`; removed
  `bin/{slstatus,nsxiv,super-tap-suckless.py,dmenu-apps.sh,dmenu-clip.sh,
  dmenu-power.sh}` + `man/man1/nsxiv.1` + `share/doc/nsxiv/`; commit `8a2a988`
  (`git diff 658f537 HEAD` empty, live hashes match baseline blobs).
- `~/.xinitrc` rebuilt from `.bak-pre-suckless-20260919` (suckless hunks
  dropped, user-s6-scandir block kept), installed `btw:btw 0755`.
- `hotkeys.toml` / `autostart.sh` restored from `.bak-pre-suckless-*` copies.
- `slstatus-widgets.sh` deleted; `stalonetray 1.5.0-1` removed via `pacman -R`.
- X restarted (old Xorg 20540 → new 29496 via s6 tty1 longrun → agetty autologin
  → zsh .zprofile `exec startx`). Pre-bar session back: `dwm` (baseline
  `8217235bfec26972`), `quickshell --path ~/.config/quickshell/shell.qml
  --no-duplicate` (started by autostart.sh), panel `0x120000e "quickshell"
  1920x30+0+0`, `picom`, `super-tap-c`, 3× `dwm-quickshell-state watch`,
  `dwm-lock-watch`; no slstatus / stalonetray.

Deliberately kept: the `slbar` + `dwmbar2` worktrees remain mounted (`syswork
list`) so the bar work is re-appliable; `~/build/{dwm-scratch,slstatus,st,nsxiv,
dmenu-flexipatch}` sources (dwm-scratch `dwm.c` still carries the bar patch,
backup `dwm.c.bak-pre-nativebar-20260919`); `suckless/notes/*`.
`syswork apply slbar --yes` would bring the bar session back.

## 2026-09-20 — chadwm is the live session (HyDE keymap port)

Moved to `https://github.com/siduck/chadwm` in full, with st as the terminal and the
keybinds of the author's Hyprland setup (https://github.com/24bit192kHz/HyDE). Done the
same day.

What chadwm is: dwm 6.5 fork (vanilla bar with status2d colours, tag preview,
monocle tab bar, vanitygaps, fibonacci/gaplessgrid/grid layouts, movestack,
shiftview) fed by `scripts/bar.sh` via `xsetroot -name`. No TOML hot-reload, no
themes.toml, no quickshell bridge — the titus layer stays installed but dormant.

Where things live:
- Clone `~/build/chadwm` (git, depth 1); `~/.config/chadwm -> ~/build/chadwm`
  (scripts hardcode that path); `~/.local/bin/chadwm -> ~/build/chadwm/chadwm/dwm`.
- Session: `~/.xinitrc` (rewritten, backup `.xinitrc.bak-pre-chadwm-20260920`)
  keeps boot lock/env/theme-env/wallpaper paint/bright/dbus/pulseaudio/user-s6
  scandir, then `exec ~/.config/chadwm/scripts/run.sh`. run.sh does xrdb +
  cursor Xresources, screen-power policy, `dwm-xsettings`, `dwm-polkit`,
  `dwm-lock-watch`, `bright restore`, wallpaper (from `wallpaper.conf`, else
  `~/Pictures/wallpapers/wallpaper.png`), `xset r`, `picom`, `bar.sh`, then the WM loop.
- Packages added: `dash`, `xorg-xsetroot`, `xorg-xbacklight`. `pacman-contrib`
  deliberately NOT installed (conflicts with the standalone `rankmirrors`
  package on `/usr/bin/rankmirrors`); `bar.sh` uses `pacman -Qu | wc -l`.
- Box edits in the tree (`corpus/configs/chadwm/chadwm-box.patch`): `config.def.h`
  keymap + 9 numeric tags (+ `tagschemes[]` extended to 9 — it is indexed by tag),
  rofi launcher instead of eww, `light_up/down` → `~/.local/bin/bright`,
  `run.sh`/`bar.sh` paths, and two `dwm.c` patches (below).

Keymap port (HyDE → chadwm), full list `~/.config/chadwm/keybinds.txt`:
- HyDE combos kept 1:1 where dwm has a twin: Super+Q/W/F/L/G/Delete,
  Alt+F4, Shift+F11, Ctrl+Alt+Delete (rofi power menu → loginctl), arrows
  (focus/resize/move), Super+T/Return (st), Super+Alt+T (dwm-terminal),
  Super+E/B, Ctrl+Shift+Escape (st -e btop), Super+A/Tab/Shift+E/Slash/Comma/
  Period/V/Shift+V/Shift+A (rofi menus, rofimoji), screenshots (Super+P/
  Ctrl+P/Alt+P, Print, Super+U, Super+O OCR, Super+Shift+P colour pick →
  new `scripts/colorpick.sh`), hardware keys (F9–F12, XF86Audio*/MonBrightness*),
  Super+F1 songid, Super+Ctrl+M (`active-audio mute`), Super+K kbd-lang,
  Super+D dictation placeholder, Super+S / Super+Alt+T / Super+Shift+S /
  Super+Alt+S → special workspace (see patch 3 below; HyDE scratchpad), Super+Shift+W wallpaper picker (`scripts/wallpaper-select.sh`),
  Super+Alt+Up/Down bar theme cycle (`scripts/bar-theme.sh`), Super+Shift+T
  theme picker (`scripts/theme-select.sh`), tags Super+1..9 (+Shift follow,
  +Alt silent, +Ctrl toggle), Super+Ctrl+Left/Right relative tag, wheel on
  bar/root = prev/next tag.
- HyDE binds with no X11/dwm twin, dropped and documented: pin window,
  Super+C center / Super+Shift+C resize-30 scripts, hyprshaderd dimming
  (Super+F11/F12), waybar-hide variants, hyprlock layouts, Super+Alt+N
  earth-native, `Super+Z/X` drag keys (mouse moveorplace/resizemouse covers it).
- chadwm-native controls kept on keys the HyDE map leaves free (layouts, gaps,
  borders, nmaster, togglebar on Super+Shift+B, restart on Super+Shift+R).

`dwm.c` patches (both required by this box, in `chadwm-box.patch`):
1. `keypress()` matches the event keycode against the keysym of **every**
   keyboard group (was group 0 only).
2. `grabkeys()` scans keycodes × groups with per-(keycode,mask) dedup instead of
   `XGetKeyboardMapping` group 0.
   Reason: the session runs `ara,us` with Arabic first (`~/.config/dwm-titus/
   kbd-lang` = `ara,us`), so group 0 has no Latin keysyms — stock dwm never
   grabbed `Super+T`, `Super+Shift+B`, `Super+Shift+R`, … and letter binds were
   dead. Verified live 2026-09-20 with `ara,us` active: `Super+T` spawns st,
   `Super+Shift+B` hides/shows the bar, digits/arrows/shiftview work.
3. Hyprland-style **special workspace** ("scratchpad") added. chadwm ships only
   `hidewin`/`restorewin`, which is not what `Super+S` does on that
   Hyprland: there it is `togglespecialworkspace` — a hidden workspace that
   floats *above* whatever workspace you are on and toggles from anywhere
   (Hyprland dispatchers doc: `togglespecialworkspace [name]` toggles a special
   workspace on/off; `movetoworkspace special` / `movetoworkspacesilent special`
   send windows there; "a special workspace floats on top of your current
   workspace and can be toggled on and off"). Implementation here:
   `scratchtag = 1u << LENGTH(tags)` (bit 9, outside `TAGMASK`), a `scratchshown`
   global, and `ISVISIBLE()` OR-ing `scratchshown && (tags & scratchtag)` — so
   the overlay shows above *every* tag, never marks a tag occupied (drawbar
   occupancy + tag preview skip scratch clients) and is skipped by `nexttiled()`.
   `manage()` parks clients whose class/instance/title contains "scratchpad"
   (e.g. `st -c scratchpad`) floating + centered, hidden until revealed.
   Binds: `Super+S` `togglescratch` (show/hide), `Super+Alt+T` `scratchterm`
   (spawn `st -c scratchpad` if the special workspace is empty, else toggle),
   `Super+Shift+S` `scratchsend {.i=1}` (send focused window + follow),
   `Super+Alt+S` `scratchsend {.i=0}` (send silently); a tiled window sent in is
   resized to 70% of the work area and centered; `Super+Shift+1..9` moves it back
   out to a normal tag (`tag()` assigns a fresh mask, dropping the scratch bit).
   Verified live 2026-09-20: overlay drawn above the tiled layer (screenshot
   `/tmp/scratch-overlay.png`), stays visible while switching tags, hide/show via
   both `Super+S` and `Super+Alt+T`, and a send → tag-switch → move-back round trip
   on throwaway `st` windows.

Verified live (Xorg :0, 2026-09-20 09:17–09:27): chadwm + bar.sh + picom, no
quickshell/slstatus/stalonetray; bar shows `1..9 | []= | title | 0 updates CPU …
BAT … Connected HH:MM` with status2d pills (screenshot evidence in session
notes); `_NET_NUMBER_OF_DESKTOPS=9`; Super+2/1 tag switch; Super+Ctrl+Right →
tag+1 (sign verified, was inverted first); bar-theme cycle tested end-to-end on
throwaway `Xvfb :9` (bar.sh line 9 + `themes/<name>.h` include + rebuild +
bar.sh restart, reverted to tundra).

Revert path: restore `~/.xinitrc.bak-pre-chadwm-20260920` (titus + quickshell
session), `pkill -x chadwm`, relog — `/usr/local/bin/dwm` (titus) is untouched,
and quickshell/titus configs are intact. chadwm is not installed into
`/usr/local` at all.

## 2026-09-20 — keybind audit + grayscale dark theme

A full keybind audit, then a grayscale dark theme with no accent colour.

Audit (4 static scouts + 2 dynamic harnesses on throwaway Xvfb displays; live
session untouched except for the final probes). Findings and fixes:

1. **grabkeys() 16-slot dedup bug (mine).** The dedup array `grabbed[256][16]`
   silently dropped bindings on keycodes with >16 distinct masks: `Super+Alt+1..9`
   (silent move), `Super+Ctrl+T` (togglegaps), `Super+Ctrl+Shift+I/O` and part of
   `Super+Alt+6..9` never got grabbed. Fixed by dropping the array entirely and
   running the grab loop under `XSetErrorHandler(xerrordummy)` (the same trick
   `killclient` uses) so duplicate grabs can't hit `xerror()`'s fatal path.
   Verified live: `Super+Ctrl+T` toggles gaps again, `Super+Alt+1..2` moves windows
   between tags without switching the view.
2. **Shadowed silent moves.** `Super+Alt+6..9` was both gap trims and silent tag
   moves (first entry wins). Gap trims moved to `Super+Ctrl+Alt+6..9` (+Shift).
3. **SILENTTAG semantics.** Now uses `tag` (client's tags change, view stays) —
   previously `toggletag`, which *added* a tag instead of moving.
4. **Super+M** pointed at `layouts[2]` (spiral) while the cheat sheet said monocle →
   now `layouts[1]`. Verified live (939px → full-width overlap in monocle).
5. **OCR bind was a silent no-op** — `tesseract` had no `eng`/`ara` data. Installed
   `tesseract-data-eng` + `tesseract-data-ara` (`tesseract --list-langs` now lists
   afr, ara, eng, osd).
6. **Scratchpad hardening** from the patch review: `restack()` re-raises overlay
   windows whenever `scratchshown` (they used to sink under tiled clients after a
   focus change/tag switch), and `toggletag` on an overlay window now takes it out of
   the special workspace instead of leaving a hybrid scratch|tag client.
7. **Launcher glyph** was an empty string (invisible click target) → `≡`.
8. **Super+scroll** added with MODKEY held, 1:1 with the HyDE map (mod-less scroll
   kept as a convenience).
10. **movestack() segfault (the reason the session kept dying).** Upstream
    `movestack` dereferences `selmon->sel` unguarded because it was only ever
    called from the tab-bar buttons. Bound to keys (`Super+Ctrl+H/L`,
    `Super+Ctrl+Shift+Up/Down`) it crashed the WM on an empty workspace — dmesg
    shows 5 identical `dwm[…]: segfault at 190 … in dwm[14189]` between 10:29 and
    10:51, and each crash ended the X session (run.sh breaks on non-zero exit).
    Fixed with an early `if (!selmon || !selmon->sel) return;` in
    `chadwm/movestack.c` (box patch). No segfault since 11:42.
11. **Operational note:** `make clean` in `~/build/chadwm/chadwm` deletes the WM
    binary that `~/.local/bin/chadwm` points at; `run.sh`'s `while type chadwm`
    loop then fails immediately and the tty1 session enters a fast restart loop
    (observed once during the audit). Always `make` before/after touching that
    tree, or the session cannot come back.
12. **Dynamic audit (all 159 key rows).** An instrumented `keypress()` (logging
    `MATCH <keysym> <state> i=<index> kc=<keycode>`) was built in a copy at
    `/tmp/kbaudit3` and run on throwaway `Xvfb :8`; every combo was injected with
    `xdotool` by X keycode. Results (evidence
    `corpus/configs/chadwm/keybind-audit-20260920.json`):
    - US layout: 147/149 matched the expected `keys[]` index on the first pass;
      both misses were harness artifacts (F10 raced a client that still held the
      keyboard; keycode 9 must be injected by name because xdotool reads
      one-digit numbers as keysyms) and both matched on retry → 149/149.
    - `ara,us` (Arabic first, this box's layout): **149/149 matched** — the
      layout-independent `keypress`/`grabkeys` patch holds.
    - 10 XF86 keysyms (audio/brightness) are absent from the Xvfb keymap; after
      extending the keymap with `X11`-style keysym numbers, all 10 matched their
      expected entries too.
    All 159 rows therefore dispatch the entry they should; no shadowed or dead
    bindings remain.

Cheat sheet (`~/.config/chadwm/keybinds.txt`) gained the missing Theming and
   Mouse sections plus the layout/gap/border variants the audit found undocumented.

Theme — `grayscale` (nightly dark, no accent colour), replacing `tundra`.
Two revisions: first mid-gray (rejected as "too dark some
parts, too white other parts"), now proper nightly dark with higher contrast:
- `themes/grayscale.h`: surfaces `#101010`, raised surface/panel `#1E1E1E`,
  unfocused border `#3A3A3A`, bar text `#E4E4E4`, selected text `#F0F0F0`,
  focused border `#EDEDED`, occupied tags descending `#EDEDED`→`#A8A8A8`,
  vacant tags `#6E6E6E`, alert slot `#C8C8C8`.
- `scripts/bar_themes/grayscale`: status2d colours matching (dark blocks, light
  text; `#101010`/`#1E1E1E`/`#B8B8B8`/`#CFCFCF`/`#E4E4E4`).
- `SchemeTagSel` (active workspace) = `#E4E4E4` block with `#101010` text — the
  light tile that pops on the dark bar; vacant tags use muted `#6E6E6E`.
- Verified by screenshot on the live session and via root `WM_NAME` palette
  (`#101010 #1E1E1E #909090 #A8A8A8 #B8B8B8 #CFCFCF #E4E4E4`) — no hue anywhere.

Rofi (launcher/run/window/filebrowser) now matches the session (before: a theme
different from the bar's, and no icons):
- `~/.config/rofi/config.rasi` (new): `drun,run,window,filebrowser` modes, font
  `JetBrainsMono Nerd Font Mono 11` (the bar font family), `show-icons: true`,
  `icon-theme: matefaenzadark` (the session's GTK icon theme), 2 columns, 8
  lines, `drun-display-format: "{icon} {name}"`, `@theme grayscale.rasi`.
- `~/.config/rofi/grayscale.rasi` (new): window/rows `#101010`, input bar
  `#1E1E1E`, selection `#2A2A2A` with `#F0F0F0` text, prompt pill `#E4E4E4`
  with dark text, border `#3A3A3A`, 8px radius.
- Verified by rendering the same config on a throwaway `Xvfb :4` and by opening
  it on the live session: dark rows, light text, one icon per entry, selection
  slightly lighter — consistent with the bar (evidence
  `corpus/configs/chadwm/rofi-theme-evidence.png`).
- Note: rofi reads `~/.config/rofi/config.rasi`; a `@theme` path is best given
  absolute or relative-to-config (the `~/` form resolves unpredictably).

## 2026-09-20 — HyDE action parity

The keymap audit proved every combo *dispatches*; this pass made the actions
behave like their Hyprland counterparts instead of placeholders:

- **OCR (Super+O, and Super+Ctrl+S from the HyDE lua extras)** now runs
  `scripts/ocr.sh`: maim region select → `tesseract -l eng+ara` → clipboard +
  notification with the text (or "no text found"). Previously an inline pipeline
  with no feedback, so it looked dead. Verified on a captured region
  (`ocr.sh --file` → "btw@artix:~" on the clipboard).
- **Super+C centre** (`centerwin()` added): floats the focused window if tiled and
  centres it on the work area. Verified on Xvfb (939px tiled → centred 487px).
- **Super+Shift+C resize** (`resizepct()` added, arg = percent): 30% of the work
  area, centred. Verified: 570x295 @672,412.
- **Super+Shift+F pin** (`alwaysontop` field + `togglepin()` + restack honours it):
  keeps the window above the tiling layer; verified — the pinned window stayed
  topmost after a new client spawned.
- **Super+F11/F12 dimmer/brighter** → `scripts/dim.sh` (xrandr `--brightness`,
  clamped 0.30–1.00, state in `$XDG_RUNTIME_DIR/chadwm-dim`); verified eDP1
  0.95 → reset 1.00.
- **Super+Ctrl+Down** nearest empty workspace (`viewempty()`); verified desktop
  0 → 1 on Xvfb.
- **Super+Ctrl+Alt+Left/Right** move the focused window one tag back/forward
  silently (`tagrel()`); verified view unchanged while the client moved.
- **Brightness: one step per tap, rate-adaptive ramp while held — read from the
  kernel** (the goal: smooth, not jittery; a full sweep took about
  10 s). Root cause found by emulating the key with a
  `uinput` virtual keyboard: this EC repeats the brightness keys as **discrete
  press/release pulses at ~10 Hz** (5 ms down), not as a continuous key-down.
  Every earlier design applied one step per detected press — i.e. 10 steps/s =
  ~10 s for a full sweep ✓ exactly the reported behaviour — and X-event designs
  additionally jittered because the server's own repeat also arrives as
  press/release pairs.
  **Measured on the real key** (BRIGHT_DEBUG log, Fn+brightness held):
  the EC emits press/release pairs with **0.26–0.52 s gaps and ~0.3 ms presses**
  — 2–4 pulses/s. That is why one-step-per-pulse was ~10 s per sweep, and it is
  why every fixed "same-hold" window failed for one side or the other.
  Final behaviour (brightness moves exactly as long as the key is held): the
  daemon tracks the pulse train, keeps the latch at
  `1.2 × the widest of the last 3 gaps`, advances `STEPS_PER_PULSE` (20) steps
  per pulse period spread evenly, **tapers to 15 % as soon as a pulse is overdue**
  (`stale > max_recent_gap`) and ends within ~1.2 × the pulse gap after the last
  one — so movement ends with the hold instead of after it. A press after >1 s of
  silence resyncs the level first (another writer may have changed it).
  Verified against a `uinput` virtual keyboard replaying the measured cadence:
  tap 50 → 51 (exactly one step); 1.0 s hold 80 → 12 with the value unchanged
  1.2 s later (no over-run); 2.5 s hold → floor; fixed 300 ms and 500 ms pulse
  trains → floor. Continuous (non-pulsing) holds still ramp at the device limit.
  Earlier attempt notes: XGrabKey is unusable here (python-xlib 0.33 has no
  `Display.grab_key`/`send_request`), X keymap polling misses the 0.3 ms pulses,
  and an average over the last 8 gaps was poisoned by gaps from earlier holds
  (fixed to the last 3). Env knobs: BRIGHT_STEP, BRIGHT_HOLD, STEPS_PER_PULSE,
  BRIGHT_DEV, BRIGHT_STATE, BRIGHT_DEBUG/BRIGHT_TRACE for diagnosis.
- Still no X11 twin (documented): hyprshaderd shaders, animations, wallbash, hyprlock layouts, earth-native, `Super+0`
  workspace 10 (dwm keeps 0 = all tags), `Super+J` split toggle (layouts cover it),
  `Super+Z/X` keyboard drag (mouse moveorplace/resizemouse covers it).

## Open questions

- Which autostart actually runs live? `~/.local/share/dwm` and `~/.dwm` are both
  absent, so `runautostart()` should no-op — yet quickshell, `dwm-lock-watch` (x2),
  `dwm-quickshell-state/network/controls` ARE running. Who launched them?
  Check `XDG_DATA_HOME`, `dwmdir` constant, or a patched autostart path.
- `dwm-status` appears in `autostart.sh` but NOT in live `ps` — is the root-window
  status text stale, or does Quickshell no longer need it?
- Who loads `~/.Xresources` / `cursor.Xresources`? No `xrdb` call in `~/.xinitrc`,
  `autostart.sh`, or `dwm-xsettings` grep hits.
- `dwm-lock` references `dwm-lock-bright` but `/usr/local/bin/dwm-lock-bright`
  does not exist — dead branch or pending install?
- Wallpaper authority: `wallpaper.conf` (`/home/btw/Pictures/wallpapers/wallpaper.png`) vs
  `dwm-titus-session` (`Pictures/t480-exploded.png`) vs autostart fallback
  (`Pictures/backgrounds`) — plus a duplicate PNG in `/home/btw/Pictures/wallpapers/`.
- Artix/s6 vs Fedora/systemd: `systemctl --user`, `wm-graphical-session.service`
  in autostart — inert or shimmed? `install.sh`/`dev-sync-install.sh` also assume
  Fedora (`/etc/os-release` says Artix) — install path unclear.
- Reinstall pending: live dwm == pre-scratch backup; when is the `togglescratch`
  worktree change (`M dwm.c`) meant to be built + installed via
  `scripts/dev-sync-install.sh`?
- `STATUSBAR "dwmblocks"` define vs actual `dwm-status` publisher — leftover?
- Two `dwm-lock-watch` + three `dwm-quickshell-state watch` instances — duplicates
  from session restarts (autostart claims singleton guards)?

## 2026-09-24 — Session locker cutover

- The live chadwm `Super+L` binding is direct `dwm-lock`; the old Titus note claiming `loginctl lock-session` is not the current chadwm path.
- `dwm-lock` now has one primary black `slock-ly` UI with separate password/fingerprint PAM services, masked input, a minimal centered card, and top-right Suspend/Reboot/Shutdown actions. Reboot and shutdown require a second click; the power child drops to the session user before `loginctl`.
- The old light-locker/XDG/desktop-saver fallback cascade is removed from the canonical launcher. `dwm-lock-watch` is singleton-guarded with a trusted PATH and a fixed helper; sleep locking routes through the same helper.
- The blue-on-typing behavior was the fullscreen `[INPUT] = "#005577"` state. The maintained source sets all four state backgrounds to black, so input feedback is only masked bullets and status text. Xvfb visual captures at 1920×1080 confirmed black outer edges in idle and typed states.
- Password and fingerprint authentication are now separate transactions. Typed password uses `slock-password`; automatic biometric attempts use `slock-finger`; the setuid binary no longer accepts user-controlled PAM service overrides. Password PAM includes the GNOME Keyring token handoff, but an independently encrypted keyring cannot be unlocked by a fingerprint alone.
- Verification: read-only review completed; final C source compiled with `-Wall -Wextra`; shell syntax passed; rebuilt setuid binary started under Xvfb as eUID 0 without the earlier PAM response double-free crash. A real password and fingerprint unlock remain to be tested on the hardware.
