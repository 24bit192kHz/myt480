#!/bin/sh
# rofi clipboard: xclip-backed history (greenclip unavailable, no ghc)
HIST="$HOME/.cache/rofi-clip-history"
mkdir -p "$(dirname "$HIST")"; touch "$HIST"
case "${1:-menu}" in
  pick)
    cur=$(xclip -o -selection clipboard 2>/dev/null | head -c 200)
    [ -n "$cur" ] && { grep -vxF "$cur" "$HIST" 2>/dev/null > "$HIST.tmp"; { printf '%s\n' "$cur"; cat "$HIST.tmp"; } > "$HIST"; rm -f "$HIST.tmp"; }
    sel=$(tac "$HIST" 2>/dev/null | rofi -dmenu -p clipboard -i)
    [ -n "$sel" ] && printf '%s' "$sel" | xclip -selection clipboard
    ;;
  menu|*)
    rofi -modi clipboard:"$0 _list" -show clipboard
    ;;
  _list)
    tac "$HIST" 2>/dev/null
    ;;
esac
