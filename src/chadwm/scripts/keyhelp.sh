#!/bin/sh
# keyhelp.sh — Super+? cheat sheet. chadwm pipes its compiled keymap in
# (keyhelp.c, same TSV as `chadwm -k`); bar clicks come from the comments in
# barclick.sh. Rows with the same modifiers + description merge (Super+arrows,
# Super+1..0), sections are packed whole into columns, colours come from
# ~/.config/theme/palette.conf, layout from rofi/keyhelp.rasi.
# Typing filters (section names match too); Enter runs the selected key
# through `chadwm -x N` (CHADWM overrides the binary, for tests).
# Usage: keyhelp.sh [--text]   (stdin: keymap TSV; a terminal => chadwm -k)

S=$HOME/.config/chadwm
[ "$1" = --text ] && mode=text || mode=rofi
[ $mode = rofi ] && pkill -x rofi && exit 0

run=${XDG_RUNTIME_DIR:-/tmp}/chadwm-keyhelp
mkdir -p "$run"
if [ -t 0 ]; then chadwm -k >"$run/keys.tsv"; else cat >"$run/keys.tsv"; fi
grep -q '^K' "$run/keys.tsv" || {
	notify-send "Super+?" "chadwm sent no keymap: restart it (Super+Shift+R)"; exit 1; }

# shellcheck disable=SC1091
. "$HOME/.config/theme/palette.conf"

# Screen -> columns and lines. Measured for keyhelp.rasi (JetBrainsMono NF 10
# at 96 dpi): 8 px per character, 21 px per line, 110 px of window chrome,
# 22 px of element padding per column.
eval "$(xwininfo -root 2>/dev/null | awk '/Width:/{print "sw="$2} /Height:/{print "sh="$2}')"
sw=${sw:-1920} sh=${sh:-1080}
cols=$(( sw >= 1800 ? 4 : sw >= 1300 ? 3 : 2 ))
cw=$(( (sw - 100 - cols * 22) / cols / 8 )); [ $cw -gt 60 ] && cw=60
maxl=$(( (sh - 110) / 21 ))

{
	cat "$run/keys.tsv"
	# not in keys[] / buttons[]: Fn brightness (kernel blkeys / brightd), the
	# bar's launcher glyph (config.h launchers[]) and the gap drag that
	# buttonpress() handles itself (HyDE resize_on_border)
	printf 'E\tMedia\t\tXF86MonBrightnessUp\tbrightness up / down\n'
	printf 'E\tMedia\t\tXF86MonBrightnessDown\tbrightness up / down\n'
	printf 'B\t\t1\tgap\tdrag to resize that edge\n'
	printf 'C\t0:1\t≡ launcher\tapp launcher\n'
	# barclick.sh "N:B) ... # icon module: action"
	sed -n 's/^\([0-9*:|]*\)).*#  *\([^:]*\): *\(.*\)$/C\t\1\t\2\t\3/p' "$S/scripts/barclick.sh"
} | LC_ALL=en_US.UTF-8 gawk -F'\t' -v mode=$mode -v cols=$cols -v cw=$cw -v maxl=$maxl \
	-v act="$run/actions" -v info="$run/layout" \
	-v cchip="$BORDER" -v cchipfg="$FG_BRIGHT" -v cmod="$GRAY_LO2" -v cdesc="$FG" -v cplace="$DIM" \
	-v chead="$FG_BRIGHT" -v cicon="$ACCENT" -v crule="$SELECTED" -v cmute="$MUTED" '
function esc(s) { gsub(/&/, "\\&amp;", s); gsub(/</, "\\&lt;", s); gsub(/>/, "\\&gt;", s); return s }
function span(color, s, extra) { return "<span foreground=\"" color "\"" extra ">" esc(s) "</span>" }
function pad(n) { return n > 0 ? sprintf("%" n "s", "") : "" }
function addsec(icon, name) { ns++; sicon[ns] = icon; sname[ns] = name; secid[name] = ns; return ns }
# one row per (section, kind, place, description); one combo per modifier set
function addrow(sec, kind, place, desc, mods, key,    r, c) {
	r = sec SUBSEP kind SUBSEP place SUBSEP desc
	if (!(r in rowid)) {
		rowid[r] = ++nr; rkind[nr] = kind; rplace[nr] = place; rdesc[nr] = desc
		srows[sec, ++srn[sec]] = nr
	}
	r = rowid[r]
	if (!((r, mods) in comboid)) { comboid[r, mods] = ++rnc[r]; cmods[r, rnc[r]] = mods }
	c = comboid[r, mods]
	if (!((r, c, key) in haskey)) { haskey[r, c, key] = 1; ckeys[r, c, ++cnk[r, c]] = key }
	LR = r; LC = c
}
BEGIN {
	n = split("Left ←|Right →|Up ↑|Down ↓|slash /|comma ,|period .|minus -|equal =|space Space|" \
	          "Return Enter|Escape Esc|Delete Del|Print PrtSc|" \
	          "XF86AudioRaiseVolume 󰝝|XF86AudioLowerVolume 󰝞|XF86AudioMute 󰖁|XF86AudioMicMute 󰍭|" \
	          "XF86AudioPlay 󰐊|XF86AudioPause 󰏤|XF86AudioNext 󰒭|XF86AudioPrev 󰒮|" \
	          "XF86MonBrightnessUp 󰃠|XF86MonBrightnessDown 󰃞", kv, "|")
	for (i = 1; i <= n; i++) { split(kv[i], p, " "); pretty[p[1]] = p[2] }
	split("LMB|MMB|RMB|󱕑|󱕐", bname, "|")
	n = split("window gap desktop tags layout title status tab tab-prev tab-next tab-close", p, " ")
	for (i = 1; i <= n; i++) porder[p[i]] = i
	placename["tab-prev"] = "tab ◂"; placename["tab-next"] = "tab ▸"; placename["tab-close"] = "tab ✕"
	cur = addsec("", "Other")
}
$1 == "S" { cur = addsec($2, $3); next }
$1 == "K" { addrow(cur, "K", "", $4, $2, $3); kidx[LR, LC, $3] = $5; next }
$1 == "E" { addrow(($2 in secid) ? secid[$2] : cur, "E", "", $5, $3, $4); next }
$1 == "B" { if (!("Mouse" in secid)) addsec("󰍽", "Mouse"); addrow(secid["Mouse"], "B", $4, $5, $2, bname[$3]); next }
$1 == "C" {
	if (!("Bar" in secid)) addsec("󰄨", "Bar")
	n = split($2, labs, "|")
	for (i = 1; i <= n; i++) { split(labs[i], mb, ":"); addrow(secid["Bar"], "C", $3, $4, "", mb[2] == "*" ? "any" : bname[mb[2]]) }
	next
}
function placeof(r) { return (rplace[r] in placename) ? placename[rplace[r]] : rplace[r] }
# chips of one combo into K[]: a full arrow set, a digit run and both wheel
# directions collapse into one chip
function keylist(r, c,    n, i, j, k, set, lo, hi) {
	delete K; n = cnk[r, c]; set = ""
	for (i = 1; i <= n; i++) set = set " " ckeys[r, c, i] " "
	if (n == 4 && set ~ / Left / && set ~ / Right / && set ~ / Up / && set ~ / Down /) { K[1] = "←→↑↓"; return 1 }
	if (n >= 3 && set ~ /^( [0-9] )+$/) {
		lo = ckeys[r, c, 1]; hi = ckeys[r, c, n]
		if (n == 10 && lo == 1 && hi == 0) { K[1] = "1…0"; return 1 }
		if (hi - lo == n - 1) { K[1] = lo "…" hi; return 1 }
	}
	j = 0
	for (i = 1; i <= n; i++) {
		k = ckeys[r, c, i]
		if (k == "󱕐" && set ~ / 󱕑 /) continue  # wheel up + down: one 󱕒 chip
		K[++j] = (k == "󱕑" && set ~ / 󱕐 /) ? "󱕒" : (rkind[r] == "B" || rkind[r] == "C") ? k : \
		         (k in pretty) ? pretty[k] : length(k) == 1 ? toupper(k) : k
	}
	return j
}
# combos c1..c2 of row r -> markup M, plain text P, width W (chips are padded)
function renderkeys(r, c1, c2,    c, i, n, m, mk, pl, w) {
	mk = pl = ""; w = 0
	for (c = c1; c <= c2; c++) {
		if (c > c1) { mk = mk span(cmute, " · "); pl = pl " · "; w += 3 }
		m = cmods[r, c]; n = keylist(r, c)
		# Super+Shift+/ is Super+?
		if (n == 1 && ckeys[r, c, 1] == "slash" && m ~ /Shift/) { sub(/\+?Shift/, "", m); K[1] = "?" }
		gsub(/\+/, " ", m)
		if (m != "") { mk = mk span(cmod, m) " "; pl = pl m " "; w += length(m) + 1 }
		for (i = 1; i <= n; i++) {
			if (i > 1) { mk = mk span(cmute, "/"); pl = pl "/"; w++ }
			mk = mk "<span background=\"" cchip "\" foreground=\"" cchipfg "\" font_weight=\"bold\"> " esc(K[i]) " </span>"
			pl = pl K[i]; w += length(K[i]) + 2
		}
	}
	M = mk; P = pl; W = w
}
# a row that is too wide for the key column shows one combo per line
function nlines(r) { return split1[r] ? rnc[r] : 1 }
function renderline(r, c) { if (split1[r]) renderkeys(r, c, c); else renderkeys(r, 1, rnc[r]) }
# one display line: key markup/plain/width, place, description, Enter action
function addline(s, km, kp, w, place, dm, dp, a,    j) {
	j = ++nl[s]; LM[s, j] = km; LP[s, j] = kp; LW[s, j] = w
	LPL[s, j] = place; LDM[s, j] = dm; LDP[s, j] = dp; LA[s, j] = a
	if (w <= KWMAX && w > kw[s]) kw[s] = w
	if (length(place) > pw[s]) pw[s] = length(place)
}
function emit(plain, disp, nonsel, action) {
	# headers / spacers: "active" rows, drawn without a selection highlight
	printf "%s\0display\x1f%s%s\n", plain, disp, nonsel ? "\x1fnonselectable\x1ftrue\x1factive\x1ftrue" : ""
	print action > act
}
function emitrule(s,    rule, hint) {
	hint = shint[s]; rule = cw - length(sname[s]) - length(hint) - 6
	# plain text " ": headers drop out as soon as the filter is not empty
	emit(" ", span(cicon, sicon[s]) "  " span(chead, sname[s], " font_weight=\"bold\"") " " \
	     span(crule, rule > 0 ? gensub(/ /, "─", "g", pad(rule)) : "") (hint != "" ? " " span(cmute, hint) : ""), 1, "-")
}
END {
	KWMAX = 22
	# Mouse rows grouped by place (window, desktop, tags, ...)
	s = secid["Mouse"]
	for (i = 2; i <= srn[s]; i++)
		for (j = i; j > 1 && porder[rplace[srows[s, j]]] < porder[rplace[srows[s, j - 1]]]; j--) {
			t = srows[s, j]; srows[s, j] = srows[s, j - 1]; srows[s, j - 1] = t
		}
	if ("Mouse" in secid) shint[secid["Mouse"]] = "LMB MMB RMB 󱕒 wheel"
	if ("Bar" in secid) shint[secid["Bar"]] = "click a module"
	for (s = 1; s <= ns; s++) {
		if (sname[s] == "Bar") {
			# one line per module: "module  [LMB] action  [RMB] action"
			for (i = 1; i <= srn[s]; i++) { r = srows[s, i]; if (!(rplace[r] in bseen)) { bseen[rplace[r]] = ++nmod; bmod[nmod] = rplace[r] } }
			# an action that would overflow the column wraps onto its own line
			for (m = 1; m <= nmod; m++) if (length(bmod[m]) > bw) bw = length(bmod[m])
			for (m = 1; m <= nmod; m++) {
				dm = dp = ""; dw = 0; first = 1
				for (i = 1; i <= srn[s]; i++) {
					r = srows[s, i]; if (rplace[r] != bmod[m]) continue
					renderkeys(r, 1, 1); w = W + 1 + length(rdesc[r])
					if (dw && dw + 3 + w > cw - bw - 2) {
						addline(s, first ? span(cmod, bmod[m]) : "", first ? bmod[m] : "", first ? length(bmod[m]) : 0, "", dm, dp, "-")
						dm = dp = ""; dw = 0; first = 0
					}
					dm = dm (dw ? "   " : "") M " " span(cdesc, rdesc[r]); dp = dp (dw ? "   " : "") P " " rdesc[r]
					dw += (dw ? 3 : 0) + w
				}
				addline(s, first ? span(cmod, bmod[m]) : "", first ? bmod[m] : "", first ? length(bmod[m]) : 0, "", dm, dp, "-")
			}
			continue
		}
		for (i = 1; i <= srn[s]; i++) {
			r = srows[s, i]; renderkeys(r, 1, rnc[r])
			split1[r] = (W > KWMAX && rnc[r] > 1)
			for (c = 1; c <= nlines(r); c++) {
				renderline(r, c)
				# Enter runs a key line that stands for one binding
				cc = split1[r] ? c : 1; a = "-"
				if (rkind[r] == "K" && cnk[r, cc] == 1) a = kidx[r, cc, ckeys[r, cc, 1]] "\t" rdesc[r]
				addline(s, M, P, W, placeof(r), rdesc[r] != "" ? span(cdesc, rdesc[r]) : span(cmute, "(no description in config.h)"), rdesc[r], a)
			}
		}
	}
	for (s = 1; s <= ns; s++) { size[s] = nl[s] ? nl[s] + 1 : 0; total += size[s] ? size[s] + 1 : 0 }
	printf "" > act
	if (mode == "text") {
		for (s = 1; s <= ns; s++) if (size[s]) {
			printf "%s%s  %s\n", shown++ ? "\n" : "", sicon[s], sname[s]
			for (j = 1; j <= nl[s]; j++)
				printf "  %s%s  %s%s\n", LP[s, j], pad(kw[s] - length(LP[s, j])),
				       pw[s] ? LPL[s, j] pad(pw[s] - length(LPL[s, j])) "  " : "", LDP[s, j]
		}
		exit
	}
	# fewest lines per column that keep every section whole
	for (L = int((total + cols - 1) / cols); L < maxl; L++) {
		col = 1; used = 0
		for (s = 1; s <= ns; s++) if (size[s]) {
			if (used + (used ? 1 : 0) + size[s] <= L) used += (used ? 1 : 0) + size[s]
			else { col++; used = size[s]; while (used > L) { col++; used -= L } }
		}
		if (col <= cols) break
	}
	used = 0
	for (s = 1; s <= ns; s++) if (size[s]) {
		if (used && used + 1 + size[s] > L) { while (used < L) { emit(" ", " ", 1, "-"); used++ } used = 0 }
		if (used) { emit(" ", " ", 1, "-"); used++ }
		emitrule(s); used++
		for (j = 1; j <= nl[s]; j++) {
			pl = pw[s] ? span(cplace, LPL[s, j]) pad(pw[s] - length(LPL[s, j])) "  " : ""
			emit(LP[s, j] "  " LDP[s, j] "  " LPL[s, j] "  " sname[s], LM[s, j] pad(kw[s] - LW[s, j]) "  " pl LDM[s, j], 0, LA[s, j])
			if (++used == L) { used = 0; if (j < nl[s]) { emitrule(s); used++ } }
		}
	}
	print L > info
}' >"$run/rows"

[ $mode = text ] && { cat "$run/rows"; exit 0; }

read -r lines <"$run/layout"
nbind=$(grep -c '^K' "$run/keys.tsv")
idx=$(rofi -dmenu -i -no-custom -markup-rows -no-show-icons -format i -p "󰌌  Keybindings" \
	-theme "$S/rofi/keyhelp.rasi" \
	-theme-str "textbox-hint { str: \"$nbind bindings · Enter runs the selected key · Esc closes\"; }" \
	-theme-str "listview { columns: $cols; lines: $lines; } window { width: $(( cols * (cw * 8 + 20) + (cols - 1) * 2 + 52 ))px; }" \
	<"$run/rows")
[ -n "$idx" ] || exit 0
line=$(sed -n "$((idx + 1))p" "$run/actions")
[ "$line" = - ] && exit 0
key=${line%%	*} desc=${line#*	}
case $desc in
*session*)
	[ "$(printf '󰜺  No\n󰍃  Yes\n' | rofi -dmenu -p "$desc?" -no-custom -no-show-icons -theme-str \
		'window {width: 264px;} listview {columns: 1; lines: 2;}')" = "󰍃  Yes" ] || exit 0 ;;
esac
# keys[N] runs inside chadwm (_CHADWM_KEY), independent of the keyboard layout
exec "${CHADWM:-chadwm}" -x "$key"
