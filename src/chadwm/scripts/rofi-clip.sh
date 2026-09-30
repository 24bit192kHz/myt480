#!/bin/sh
# rofi clipboard history (greenclip unavailable, no ghc).
#   daemon  record every CLIPBOARD change (clipwatch); started from run.sh
#   pick    Super+V: choose an entry, copy it back to the clipboard
#   menu    Super+Shift+V: delete an entry or clear the history
# One file per entry (multi-line clips survive intact), newest = latest mtime.
# Installed as ~/.local/bin/rofi-clip.sh (shadows the old /usr/local/bin copy).
DIR="${XDG_CACHE_HOME:-$HOME/.cache}/rofi-clip"
MAX=100
umask 077
mkdir -p "$DIR"

add() {
	# text only: skip image-only owners (maim) — xclip would hand back raw PNG
	xclip -o -selection clipboard -t TARGETS 2>/dev/null |
		grep -qxE 'UTF8_STRING|STRING|TEXT|text/plain.*' || return
	f=$(mktemp "$DIR/.new.XXXXXX") || return
	xclip -o -selection clipboard 2>/dev/null >"$f"
	if ! grep -q '[^[:space:]]' "$f" 2>/dev/null; then
		rm -f "$f"
		return
	fi
	# same content -> same name, so a re-copy just moves it to the top
	mv -f "$f" "$DIR/$(cksum <"$f" | tr ' ' _)"
	ls -t "$DIR" | tail -n +$((MAX + 1)) | while read -r old; do rm -f "$DIR/$old"; done
}

entries() { ls -t "$DIR"; }

# one rofi row per entry: first non-blank line, trimmed, with a marker if more
previews() {
	entries | while read -r f; do
		awk 'NF && !s { gsub(/^[ \t]+|[ \t]+$/, ""); s = $0 }
		     END { if (length(s) > 120) s = substr(s, 1, 120) "…"
		           if (NR > 1) s = s "  ⏎"; print s }' "$DIR/$f"
	done
}

nth() { entries | sed -n "$(($1 + 1))p"; }

case "${1:-pick}" in
daemon)
	exec 9>"$DIR/.lock"
	flock -n 9 || exit 0 # already running
	add
	clipwatch | while read -r _; do add; done
	;;
pick)
	[ -n "$(entries)" ] || { rofi -e "Clipboard history is empty"; exit 0; }
	i=$(previews | rofi -dmenu -i -p clipboard -format i) || exit 0
	f=$(nth "$i")
	[ -n "$f" ] && xclip -selection clipboard -i "$DIR/$f" >/dev/null
	;;
menu)
	i=$( { echo "⟨clear history⟩"; previews; } | rofi -dmenu -i -p "delete" -format i) || exit 0
	if [ "$i" = 0 ]; then
		entries | while read -r f; do rm -f "$DIR/$f"; done
	else
		f=$(nth $((i - 1)))
		[ -n "$f" ] && rm -f "$DIR/$f"
	fi
	;;
*)
	echo "usage: ${0##*/} daemon|pick|menu" >&2
	exit 2
	;;
esac
