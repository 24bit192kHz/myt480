# quickshell Desktop Shell — Recon Notes

> History: quickshell was the bar and shell of the dwm-titus desktop. The chadwm desktop
> (since 2026-09-20) does not start it.

Date: 2026-09-19 · Host: Artix ThinkPad T480 · quickshell 0.3.1 (Arch)
Config: `~/.config/quickshell/` (~24k lines, 16 dirs) · Entry: `shell.qml` (1208 lines, 34.2K)

**Status 2026-09-20: dormant.** The live session moved to chadwm (see
`notes/dwm.md`), which draws its own bar and starts no quickshell; the panel,
its providers and `dwm-quickshell-*` bridges are no longer launched. Config is
intact — restore `~/.xinitrc.bak-pre-chadwm-20260920` to bring the panel back.

Running (as of the 2026-09-19 recon): PID 1790 `quickshell --path ~/.config/quickshell/shell.qml --no-duplicate`

## Overview

dwm-titus Qt/QML desktop shell on top of dwm. quickshell replaces the old
`dwm-status` bar with a per-monitor panel plus popup windows (launcher, command
menu, network, audio/BT, control center, power, settings, health, notifications).
No dwm patch IPC — all state flows through **xprop polling/spy + helper shell
scripts + `quickshell ipc`**. QML models shell out via `Quickshell.Io Process`
using command builders in `core/Commands.qml`. Thin wrappers in
`~/.local/share/dwm-titus/scripts/dwm-{controlcenter,settings}` call
`quickshell ipc --path <shell.qml> call <target> <action>` (used by dwm keybinds).

## Entry point (shell.qml)

- `ShellRoot` instantiates one singleton-ish `Scope` model per domain
  (DwmState, LauncherModel, CommandMenuModel, PowerMenuModel/PowerModel,
  DefaultApps/Autostart/Appearance/Accessibility/PanelSettings/Network/Controls/
  Bluetooth/ControlCenter/SystemHealth/SystemManagement/Settings/NotificationModel).
- Popup exclusivity: `selectPanelPopup()` / `openCommandMenu()` close all other
  popups before opening one; `commandMenuModel.currentEntryIds` tracks which
  surfaces are open so the command menu can annotate "current" entries.
- **IpcHandlers** (the dwm-keybind API surface): `launcher`, `menu`
  (open/summon-on-focused-screen/toggle), `power`, `network`, `controls`
  (volume/mic/media/BT status), `notifications`, `controlcenter`,
  `systemhealth`, `settings` (~130 getters incl. provider states, counts,
  appearance inventory, power/autostart), `tray` (count/ids/details from
  `SystemTray.items`).
- Windows: `LauncherWindow`, `CommandMenuWindow`, `PowerMenuWindow` +
  per-screen `Variants { model: Quickshell.screens; DwmPanel {...} }` +
  `NetworkWindow`, `ControlsWindow`, `BluetoothWindow`, `ControlCenterWindow`,
  `UtilityDetailWindow`, `SystemHealthWindow`, `SettingsWindow`.
- `LazyLoader` on startup calls `networkModel/bluetoothModel/controlsModel.refresh()`.

## Module inventory

| Dir | Purpose | Key files |
|---|---|---|
| `panel/` | Per-monitor bar: logo, workspace buttons, status pills, running-apps, tray, clock, volume/net/BT/power pills → popups | `DwmPanel.qml` (417L), `WorkspaceButton.qml`, `RunningAppsArea.qml`+`RunningAppItem.qml`, `TrayArea.qml`+`TrayItem.qml`, `LogoButton.qml`, `PanelSettingsModel.qml` |
| `state/` | dwm mirror: `dwm-quickshell-state watch` stdout → `current/focused_monitor/monitor_desktops/names/occupied/apps/title/class/status`; `switchWorkspace`/`focusWindow` via wmctrl; screen↔monitor mapping incl. HiDPI pixel/logical match | `DwmState.qml` (268L) |
| `launcher/` | App launcher (fuzzy `list` over .desktop files, `launch`) + command menu (static catalog + live app entries + screenshot/power helpers + IPC actions) | `LauncherModel.qml`, `LauncherWindow.qml`, `CommandMenuModel.qml`, `CommandMenuWindow.qml`, `CommandMenuCatalog.js` (menus: root/apps/display-input/screenshots/system) |
| `controls/` | Audio (Pipewire native + `audio-snapshot/watch` fallback), volume/mic/media status, media keys via playerctl, BT adapter/devices | `ControlsModel.qml` (608L), `ControlsWindow.qml`, `BluetoothModel.qml`, `BluetoothWindow.qml` |
| `network/` | NM state: snapshot/status/devices/connections, wifi scan/connect, forget, `monitor` (nmcli), editor detect | `NetworkModel.qml`, `NetworkWindow.qml` (366L) |
| `controlcenter/` | Quick-settings hub: overview pages (appearance/themes, keybinds, sysinfo via `dwm-quickshell-controlcenter info/themes/keybinds`), theme-set, session/power actions (delegates to `powerHelperCommand` = same script) | `ControlCenterModel.qml`, `ControlCenterWindow.qml` (461L), `UtilityDetailWindow.qml` |
| `power/` | UPower-backed battery/profiles/DPMS/lock policy + power menu (lock/logout/reboot/shutdown with confirm) | `PowerModel.qml` (~600L), `PowerMenuModel.qml`, `PowerMenuWindow.qml` |
| `health/` | `dwm-system-health scan-user/scan-system`, evidence view, repair actions, card UI | `SystemHealthModel.qml`, `SystemHealthWindow.qml` (389L), `HealthCheckCard.qml` |
| `settings/` | 10-pane settings app (Appearance/Audio/BT/Defaults/Display/Input/Network/Power/System + deferred), driven by SettingsModel capability discovery over `dwm-settings-*` provider scripts | `SettingsModel.qml` (1109L), `SettingsWindow.qml` (578L), `*SettingsPane.qml`, `DisplayLayout.js` |
| `appearance/` | Theme/font/wallpaper/personalization inventory + preview/apply/reconcile; owns `themes.toml`, `font.conf`, wallpaper state; pushes colors/fonts into Theme singleton | `AppearanceModel.qml` (2249L — largest file) |
| `core/` | Shared primitives: `Theme.qml` singleton (colors/spacing/type), `Commands.qml` (all helper command builders), `ClockModel.qml`, widgets (`PanelPill`, `PanelSlider`, `PanelToggleSwitch`, `ShellSurface`, `ClickAwayPopup`, …) | `Theme.qml`, `Commands.qml`, `Icons.qml`, `ClockModel.qml` |
| `notifications/` | Native `NotificationServer` capture + popup/history windows, DND, timeout, policy | `NotificationModel.qml`, `NotificationPopupWindow.qml` (49L), `NotificationHistoryWindow.qml`, `NotificationCard.qml` |
| `defaults/` | Default-apps roles + XDG autostart entries via `dwm-default-apps`, `dwm-xdg-autostart` | `DefaultAppsModel.qml`, `AutostartModel.qml` |
| `accessibility/` | High-contrast / reduced-motion via `dwm-accessibility-settings` → `Theme.applyAccessibility` | `AccessibilityModel.qml` |
| `systemmanagement/` | Post-install system mgmt: snapshot/snapshot-core protocol over `dwm-system-management` (465K script), provider discovery, updates, regional/timezone, op tracking | `SystemManagementModel.qml` (1296L), `SystemOperationModel.qml`, `System*Protocol.js`, `SystemProviderDiscovery.qml`, `SystemUpdateDiscovery.qml` |

## dwm↔quickshell bridge mechanism

- **State out of dwm (xprop, NOT EWMH events):** `/usr/local/bin/dwm-quickshell-state`
  (5.5K) `show_state` emits `current/monitor_desktops/focused_monitor/count/names/
  occupied/fullscreen_monitors/apps/active_window/title/class/status` parsed from
  `_NET_CURRENT_DESKTOP`, `_NET_NUMBER_OF_DESKTOPS`, `_NET_DESKTOP_NAMES`,
  `_NET_ACTIVE_WINDOW`, `_NET_CLIENT_LIST` (per-window `_NET_WM_DESKTOP`,
  `_NET_WM_PID`, `WM_CLASS`, `_NET_WM_NAME`), `WM_NAME` (root status text),
  custom `_DWM_MONITOR_DESKTOPS`, `_DWM_SELECTED_MONITOR`,
  `_DWM_FULLSCREEN_MONITORS`, + `xdotool getactivewindow` fallback. `watch`
  loops `xprop -root -spy DWM_TAG_UPDATE <same props>` re-emitting full snapshot
  per event; QML `SplitParser splitMarker "\n\n"` feeds `DwmState.parseState`.
  `switch <n>` / `focus <winid>` go through **wmctrl**.
- **Control into dwm:** keybinds call `quickshell ipc` (`menu summon` uses
  `dwmState.focusedScreen()`; `dwm-quickshell-pointer command-menu` uses xdotool
  to warp the pointer into the menu window).
- **Helper scripts** (QML never calls binaries directly except via Commands.qml):
  `dwm-quickshell-launcher` (411L: desktop-file `list/launch/launch-chatgpt`),
  `dwm-quickshell-network` (404L: NM snapshot/monitor/wifi),
  `dwm-quickshell-controls` (882L: audio/BT/media, `run_parent_bound` +
  `run_bounded` timeout guards, `playerctl --follow` for media-watch),
  `dwm-quickshell-controlcenter` (1779L: sysinfo/themes/keybinds/session/power),
  `dwm-quickshell-pointer` (xdotool warp), `dwm-quickshell-version-check`
  (pins git `dacfa9d`/0.2.1 → but running quickshell is **0.3.1**).
  `Commands.helperCommand()` prefers `command -v <helper>` then
  `$data_dir/scripts/<helper>` (managed copy), except launcher/controls/etc.
  prefer managed first — so `/usr/local/bin` and
  `~/.local/share/dwm-titus/scripts/` copies can diverge (they do: state script
  differs; controls has an extra `media-watch` child pair in ps).
- **Running watchers now:** 3× `dwm-quickshell-state watch`, 2×
  `dwm-quickshell-network monitor` (`nmcli monitor` via run_parent_bound),
  2× `dwm-quickshell-controls media-watch` (`playerctl --follow`).

## Theming/appearance data flow

`dwm-settings-theme/wallpaper/font/personalization/appearance` (+`dwm-xsettings`)
→ `AppearanceModel` processes (snapshot/inventory/watch-inventory/
watch-compositor/preview-status/recovery-status, `checkedCommand` success-gated)
→ `Theme.applyAppearanceColors(colors, darkMode)` sets `bg/bar-background/
surface(-hover/-active)/border(-strong)/text(-strong/-muted)/placeholder/
accent(-secondary/-text)/success/warning/danger(-surface)`; all surfaces consume
derived semantic roles (`popupBackground`, `menuHoverBackground`,
`controlSelectedBorder`, …). Fonts via `applyFontPreferences` (family+scale,
0.8–1.5 clamp); a11y via `applyAccessibility`. `Theme.qml` hardcodes fallback
palette (dark gray bg `#1A1A1A`, white accents) + Omarchy-adapted spacing/type
scales, `panelHeight: 30`.

## Known fragilities

- **xprop polling cost:** `occupied_workspaces`/`running_apps` fork `xprop -id`
  per client-list window on *every* spy event; full snapshot re-emit per event.
- **wmctrl + xdotool hard deps** for workspace switch, window focus, active-window
  fallback, pointer warp — Wayland-incompatible, silent failures if missing.
- **Script duplication:** `/usr/local/bin/*` (unowned by pacman — manual install)
  vs `~/.local/share/dwm-titus/scripts/*` can drift; `preferManaged` flag differs
  per helper (network prefers PATH, most others prefer managed).
- **Version-check stale:** `dwm-quickshell-version-check` allows ≤0.3.x but pins
  0.2.1-git hash; running 0.3.1 passes only via the semver fallback branch.
- **Recent edits (2026-09-16 .bak trio):** `Theme.qml.bak-pre-wscolors` — added
  workspace selected/occupied colors (selected=red `danger`, follows theme);
  `DwmPanel.qml.bak-pre-occupiedcall` — identical to current (reverted or no-op);
  `dwm-quickshell-state.bak-pre-occupied` — `occupied_workspaces` changed from
  `root_property_value _NET_CLIENT_LIST | tr ','` to direct
  `xprop -root _NET_CLIENT_LIST | grep -o '0x…'` (root_property_value strips to
  first `=`, breaking multi-window lists — likely an occupied-tags fix).
- **Long-running Process hazards:** `terminatingCheckedCommand` wrapper exists
  for surface-close kill forwarding, but watchers (`audio-watch`, `nmcli
  monitor`, `playerctl --follow`) depend on `run_parent_bound` PPID checks;
  orphaned duplicates visible in ps (double network monitor + media-watch pairs).
- **QML lint suppression:** `DwmPanel.qml` disables `uncreatable-type` for
  PanelWindow; `pragma ComponentBehavior: Bound` everywhere — scope bugs surface
  only at runtime.
