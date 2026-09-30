#!/bin/sh
# wallpaper-select.sh — Super+Shift+W: pick the planet the gxwc wallpaper
# draws. A rofi strip of preview renders, four visible, alphabetical; Left /
# Right or the mouse wheel scroll, Enter applies (gxwc-wallpaper restarts
# the renderer), Escape keeps the current one. Planets live in
# ~/.local/share/gxwc/planets (installed from ~/Projects/software/gxwc/planets).

pkill -x rofi && exit 0

wp="$HOME/.local/bin/gxwc-wallpaper"
dir="$HOME/.local/share/gxwc/planets"
[ -x "$wp" ] || { notify-send "wallpaper: gxwc-wallpaper missing"; exit 1; }

current=$(cat "$HOME/.config/gxwc/planet" 2>/dev/null || echo earth)
names=$("$wp" planet | sed -n 's/.*(available: \(.*\))$/\1/p' | tr ' ' '\n')
[ -n "$names" ] || { notify-send "wallpaper: no planets installed"; exit 1; }
row=$(printf '%s\n' "$names" | grep -nx "$current" | cut -d: -f1)

sel=$(printf '%s\n' "$names" | while read -r n; do
	printf '%s\0icon\037%s\n' "$n" "$dir/$n/preview.png"
done | rofi -dmenu -p planet -no-custom -show-icons -scroll-method 1 -selected-row "$((${row:-1} - 1))" \
	-kb-move-char-back '' -kb-move-char-forward '' \
	-kb-element-prev 'Left,ISO_Left_Tab' -kb-element-next 'Right,Tab' \
	-theme-str '
		window { width: 1000px; }
		inputbar { enabled: false; }
		listview { columns: 4; lines: 1; flow: vertical; fixed-columns: true; spacing: 8px; scrollbar: false; }
		element { orientation: vertical; padding: 8px; spacing: 6px; }
		element-icon { size: 216px; }
		element-text { horizontal-align: 0.5; }
	')
[ -n "$sel" ] || exit 0
[ "$sel" = "$current" ] || "$wp" planet "$sel" >/dev/null
notify-send "Wallpaper: $sel"
