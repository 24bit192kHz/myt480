#!/bin/sh
# keybind panel: pipes binds list into fuzzel dmenu, execs selected info
cut -d"|" -f1 "$HOME/.local/bin/keys.txt" | fuzzel --dmenu --prompt="keys> "
