/* See LICENSE file for copyright and license details. */

#include <X11/XF86keysym.h>

/* appearance */
static const unsigned int borderpx  = 2;        /* HyDE border_size 2 */        /* border pixel of windows */
static const unsigned int default_border = 2;   /* to switch back to default border after dynamic border resizing via keybinds */
static const unsigned int snap      = 32;       /* snap pixel */
static const float dimspecial       = 0.5;      /* Hyprland dim_special: darken the tag under Super+S (0 = off) */
static const unsigned int gappih    = 6;        /* HyDE gaps_in 3 (per side) */       /* horiz inner gap between windows */
static const unsigned int gappiv    = 6;       /* vert inner gap between windows */
static const unsigned int gappoh    = 8;        /* HyDE gaps_out 8 */       /* horiz outer gap between windows and screen edge */
static const unsigned int gappov    = 8;       /* vert outer gap between windows and screen edge */
static const int smartgaps          = 0;        /* 1 means no outer gap when there is only one window */
static const unsigned int systraypinning = 0;   /* 0: sloppy systray follows selected monitor, >0: pin systray to monitor X */
static const unsigned int systrayspacing = 8;   /* systray spacing */
static const unsigned int systrayiconsize = 20; /* systray icon size in px */
static const int systraypinningfailfirst = 1;   /* 1: if pinning fails,display systray on the 1st monitor,False: display systray on last monitor*/
static const int showsystray        = 1;        /* 0 means no systray */
static const int showbar            = 1;        /* 0 means no bar */
static const int showtab            = showtab_auto;
static const int toptab             = 1;        /* 0 means bottom tab */
static const int floatbar           = 1;        /* 1 means the bar will float(don't have padding),0 means the bar have padding */
static const int topbar             = 1;        /* 0 means bottom bar */
static const int horizpadbar        = 5;
static const int vertpadbar         = 11;
static const int vertpadtab         = 35;
static const int horizpadtabi       = 15;
static const int horizpadtabo       = 15;
static const int scalepreview       = 4;
static const int tag_preview        = 0;        /* 1 means enable, 0 is off */
static const int colorfultag        = 1;        /* 0 means use SchemeSel for selected non vacant tag */
/* volume via scripts/vol.sh: default sink, 5% steps capped at 100%, and an
 * immediate bar refresh (SIGUSR2 to bar.sh: volume/brightness only) */
static const char *upvol[]   = { "/bin/sh", "-c", "exec \"$HOME/.config/chadwm/scripts/vol.sh\" up",   NULL };
static const char *downvol[] = { "/bin/sh", "-c", "exec \"$HOME/.config/chadwm/scripts/vol.sh\" down", NULL };
static const char *mutevol[] = { "/bin/sh", "-c", "exec \"$HOME/.config/chadwm/scripts/vol.sh\" mute", NULL };

static const int new_window_attach_on_end = 1; /* dwindle: older windows keep their place (Hyprland) */ /*  1 means the new window will attach on the end; 0 means the new window will attach on the front,default is front */
#define ICONSIZE 19   /* icon size */
#define ICONSPACING 8 /* space between icon and title */

static const char *fonts[]          = {"JetBrainsMono Nerd Font:style:Medium:size=11" ,"JetBrainsMono Nerd Font Mono:style:medium:size=19", "Droid Arabic Kufi:size=11" };

// theme
#include "themes/grayscale.h"

static const char *colors[][3]      = {
    /*                     fg       bg      border */
    [SchemeNorm]       = { gray3,   black,  gray2 },
    [SchemeSel]        = { gray4,   blue,   white },
    [SchemeTitle]      = { gray4,   black,  black }, // active window title
    [TabSel]           = { gray4,   blue,   black },
    [TabNorm]          = { gray3,   black,  black },
    [SchemeTag]        = { pink,    black,  black }, // vacant tags
    [SchemeTagSel]     = { black,   gray3,  gray3 }, // active workspace: light block, dark text
    [SchemeTag1]       = { white,   black,  black }, // occupied tags: descending grays
    [SchemeTag2]       = { orange,  black,  black },
    [SchemeTag3]       = { green,   black,  black },
    [SchemeTag4]       = { yellow,  black,  black },
    [SchemeTag5]       = { pink,    black,  black },
    [SchemeLayout]     = { gray3,   black,  black },
    [SchemeBtnPrev]    = { gray3,   black,  black },
    [SchemeBtnNext]    = { gray4,   black,  black },
    [SchemeBtnClose]   = { yellow,  black,  black },
};

/* tagging -- box: 9 numeric tags (was 5 glyphs) to match the HyDE workspace map */
static char *tags[] = {"1", "2", "3", "4", "5", "6", "7", "8", "9", "10"}; /* HyDE: 10 workspaces */

/* box: rofi replaced chadwm's optional eww launcher (eww not installed here) */
static const char* rofi_drun[] = { "rofi", "-show", "drun", NULL };

static const Launcher launchers[] = {
    /* command     name to display */
    { rofi_drun,   "≡" },
};

/* one scheme per tag; MUST keep LENGTH >= LENGTH(tags) (dwm.c indexes it by tag) */
static const int tagschemes[] = {
    SchemeTag1, SchemeTag2, SchemeTag3, SchemeTag4, SchemeTag5,
    SchemeTag1, SchemeTag2, SchemeTag3, SchemeTag4, SchemeTag5,
};

static const unsigned int ulinepad      = 5; /* horizontal padding between the underline and tag */
static const unsigned int ulinestroke   = 2; /* thickness / height of the underline */
static const unsigned int ulinevoffset  = 0; /* how far above the bottom of the bar the line should appear */
static const int ulineall               = 0; /* 1 to show underline on all tags, 0 for just the active ones */

static const Rule rules[] = {
    /* xprop(1):
     *	WM_CLASS(STRING) = instance, class
     *	WM_NAME(STRING) = title
     */
    /* class      instance    title       tags mask     iscentered   isfloating   monitor */
    { "Gimp",     NULL,       NULL,       0,            0,           1,           -1 },
    { "Firefox",  NULL,       NULL,       1 << 8,       0,           0,           -1 },
    { "eww",      NULL,       NULL,       0,            0,           1,           -1 },
};

/* layout(s) */
static const float mfact     = 0.50; /* factor of master area size [0.05..0.95] */
static const int nmaster     = 1;    /* number of clients in master area */
static const int resizehints = 0;    /* 1 means respect size hints in tiled resizals */
static const int lockfullscreen = 1; /* 1 will force focus on the fullscreen window */

#define FORCE_VSPLIT 1  /* nrowgrid layout: force two clients to always split vertically */
#include "functions.h"


static const Layout layouts[] = {
    /* symbol     arrange function */
    { "[D]",      hydwindle }, /* first entry is default: HyDE dwindle (hyde.c) */
    { "[]=",      tile },
    { "[M]",      monocle },
    { "[@]",      spiral },
    { "[\\]",     dwindle },
    { "H[]",      deck },
    { "TTT",      bstack },
    { "===",      bstackhoriz },
    { "HHH",      grid },
    { "###",      nrowgrid },
    { "---",      horizgrid },
    { ":::",      gaplessgrid },
    { "|M|",      centeredmaster },
    { ">M>",      centeredfloatingmaster },
    { "><>",      NULL },    /* no layout function means floating behavior */
    { NULL,       NULL },
};

/* key definitions */
#define MODKEY Mod4Mask
#define TAGKEYS(KEY,TAG) \
    { MODKEY,                       KEY,      view,           {.ui = 1 << TAG}, "go to workspace" }, \
    { MODKEY|ControlMask,           KEY,      toggleview,     {.ui = 1 << TAG}, "show workspace alongside" }, \
    { MODKEY|ShiftMask,             KEY,      tag,            {.ui = 1 << TAG}, "move window to workspace" }, \
    { MODKEY|ControlMask|ShiftMask, KEY,      toggletag,      {.ui = 1 << TAG}, "also show window on workspace" },
/* HyDE: Super+Alt+N moves the window to workspace N without following it
 * (dwm `tag` changes the client's tags only, so the view stays put) */
#define SILENTTAG(KEY,TAG) \
    { MODKEY|Mod1Mask,              KEY,      tag,            {.ui = 1 << TAG}, "move window to workspace" },

/* helper for spawning shell commands in the pre dwm-5.0 fashion */
#define SHCMD(cmd) { .v = (const char*[]){ "/bin/sh", "-c", cmd, NULL } }

/* Super+? cheat sheet (keyhelp.c): the last field of every key and button is
 * its description, KEYSECTION starts a group. Entries with the same modifiers
 * and description are shown as one row (Super+arrows, Super+1..0), so give
 * pairs one shared text ("previous / next ..."). Keep descriptions short;
 * the sheet has four columns. */
#define KEYSECTION(icon, name) { 0, NoSymbol, NULL, {0}, icon "\t" name },
static const char keyhelpcmd[] = "exec \"$HOME/.config/chadwm/scripts/keyhelp.sh\"";

/* Keymap ported 2026-09-20 from the owner's Hyprland map (HyDE,
 * https://github.com/24bit192kHz/HyDE, Configs/.local/share/hypr/lua/key_binds.lua).
 * Full combo list: Super+? or `chadwm -k`. HyDE's center/resize scripts
 * are native (centerwin, resizepct); unmappable HyDE binds (hyprshaderd
 * dimming, hyprlock layouts, Super+Alt+N earth-native) are dropped. */
static const Key keys[] = {
    /* modifier                         key         function        argument        description */

    KEYSECTION("󰖲", "Windows")
    { MODKEY,                           XK_q,       killclient,     {0},            "close window" },
    { Mod1Mask,                         XK_F4,      killclient,     {0},            "close window" },   /* HyDE ALT+F4 */
    { MODKEY,                           XK_w,       togglefloating, {0},            "toggle floating" },
    { MODKEY|ShiftMask,                 XK_space,   togglefloating, {0},            "toggle floating" },
    { MODKEY,                           XK_f,       togglefullscr,  {0},            "toggle fullscreen" },   /* HyDE Super+F */
    { ShiftMask,                        XK_F11,     togglefullscr,  {0},            "toggle fullscreen" },   /* HyDE SHIFT+F11 */
    { MODKEY|ShiftMask,                 XK_f,       togglepin,      {0},            "pin above tiled windows" },   /* HyDE Super+Shift+F pin window */
    { MODKEY,                           XK_c,       centerwin,      {0},            "centre window (floats it)" },   /* HyDE Super+C centre window */
    { MODKEY|ShiftMask,                 XK_c,       resizepct,      {.i = 90},      "float at 90% × 70%, centred" }, /* HyDE Super+Shift+C (resize-30.sh) */
    { MODKEY,                           XK_g,       tabmode,        {.i = -1},      "toggle tab bar (group)" }, /* HyDE group toggle -> chadwm tab bar */
    { MODKEY,                           XK_j,       togglesplit,    {0},            "flip split direction" },   /* HyDE Super+J toggle split */
    { MODKEY|ShiftMask,                 XK_o,       setcfact,       {.f =  0.00},   "reset window size factor" },

    KEYSECTION("󰆾", "Focus & move")
    { MODKEY,                           XK_Left,    focusdir,       {.i = 0},       "focus in that direction" }, /* HyDE focus left */
    { MODKEY,                           XK_Right,   focusdir,       {.i = 1},       "focus in that direction" }, /* HyDE focus right */
    { MODKEY,                           XK_Up,      focusdir,       {.i = 2},       "focus in that direction" }, /* HyDE focus up */
    { MODKEY,                           XK_Down,    focusdir,       {.i = 3},       "focus in that direction" }, /* HyDE focus down */
    { Mod1Mask,                         XK_Tab,     focusstack,     {.i = +1},      "cycle focus" }, /* HyDE ALT+TAB cycle focus */
    /* HyDE [Group Navigation] -> tab / stack order */
    { MODKEY|ControlMask,               XK_h,       focusstack,     {.i = -1},      "previous / next in group" }, /* HyDE group prev */
    { MODKEY|ControlMask,               XK_l,       focusstack,     {.i = +1},      "previous / next in group" }, /* HyDE group next */
    /* HyDE resizeactive +-30 px: tiled, the nearest divider moves that way;
     * floating, the window grows / shrinks around its centre (hyde.c) */
    { MODKEY|ShiftMask,                 XK_Left,    resizedir,      {.i = 0},       "resize: divider moves 30 px" },
    { MODKEY|ShiftMask,                 XK_Right,   resizedir,      {.i = 1},       "resize: divider moves 30 px" },
    { MODKEY|ShiftMask,                 XK_Up,      resizedir,      {.i = 2},       "resize: divider moves 30 px" },
    { MODKEY|ShiftMask,                 XK_Down,    resizedir,      {.i = 3},       "resize: divider moves 30 px" },
    /* HyDE movewindow: swap with the neighbour that way (floating: move 30 px) */
    { MODKEY|ControlMask|ShiftMask,     XK_Left,    movedir,        {.i = 0},       "move window that way" },
    { MODKEY|ControlMask|ShiftMask,     XK_Right,   movedir,        {.i = 1},       "move window that way" },
    { MODKEY|ControlMask|ShiftMask,     XK_Up,      movedir,        {.i = 2},       "move window that way" },
    { MODKEY|ControlMask|ShiftMask,     XK_Down,    movedir,        {.i = 3},       "move window that way" },
    /* HyDE Super+Z / Super+X: hold and move the mouse to drag / resize */
    { MODKEY,                           XK_z,       moveorplace,    {.i = 0},       "hold + mouse: move window" },
    { MODKEY,                           XK_x,       resizemouse,    {0},            "hold + mouse: resize window" },

    // HyDE [Launcher|Apps]
    KEYSECTION("󱓞", "Launch")
    { MODKEY,                           XK_Return,  spawn,          SHCMD("st"),    "terminal (st)" },
    { MODKEY,                           XK_t,       spawn,          SHCMD("st"),    "terminal (st)" },  /* HyDE Super+T */
    { MODKEY|Mod1Mask,                  XK_t,       droptoggle,     {0},            "dropdown terminal" }, /* HyDE dropdown terminal (pypr console) */
    { MODKEY,                           XK_e,       spawn,          SHCMD("pcmanfm-qt"), "file manager" },
    { MODKEY,                           XK_b,       spawn,          SHCMD("firefox"), "browser (Firefox)" },
    { ControlMask|ShiftMask,            XK_Escape,  spawn,          SHCMD("st -e btop"), "system monitor (btop)" },
    // HyDE [Launcher|Rofi menus]
    { MODKEY,                           XK_a,       spawn,          SHCMD("rofi -show drun"), "app launcher" },
    { MODKEY|ShiftMask,                 XK_a,       spawn,          SHCMD("rofi -show combi"), "combined launcher" },    /* HyDE select rofi launcher */
    { MODKEY|ShiftMask,                 XK_e,       spawn,          SHCMD("rofi -modes filebrowser -show filebrowser"), "file finder" }, /* HyDE file finder */
    { MODKEY,                           XK_Tab,     spawn,          SHCMD("rofi -show window"), "window switcher" }, /* HyDE Super+TAB window switcher */

    KEYSECTION("󰍜", "Tools")
    { MODKEY,                           XK_slash,   keyhelp,        {.v = keyhelpcmd}, "this cheat sheet" }, /* HyDE keybindings hint */
    { MODKEY|ShiftMask,                 XK_slash,   keyhelp,        {.v = keyhelpcmd}, "this cheat sheet" }, /* Super+? */
    { MODKEY,                           XK_comma,   spawn,          SHCMD("rofimoji --action type --selector rofi --clipboarder xclip --typer xdotool"), "emoji picker" }, /* HyDE emoji picker */
    { MODKEY,                           XK_period,  spawn,          SHCMD("~/.config/chadwm/scripts/glyph-picker.sh"), "Nerd Font glyph picker" }, /* HyDE glyph picker */
    { MODKEY,                           XK_v,       spawn,          SHCMD("rofi-clip.sh pick"), "clipboard history" },   /* HyDE clipboard */
    { MODKEY|ShiftMask,                 XK_v,       spawn,          SHCMD("rofi-clip.sh menu"), "clipboard manager" },   /* HyDE clipboard manager */
    // HyDE [Utilities]
    { MODKEY,                           XK_k,       spawn,          SHCMD("kbd-lang.sh toggle"), "switch keyboard layout" },    /* HyDE Super+K keyboard layout */
    { MODKEY,                           XK_d,       spawn,          SHCMD("notify-send Dictation 'no STT backend installed'"), "dictation (no backend yet)" }, /* HyDE Super+D dictation (box: no STT) */
    { MODKEY|Mod1Mask,                  XK_g,       spawn,          SHCMD("notify-send 'GameMode on'"), "game mode (placeholder)" }, /* HyDE Super+Alt+G (box: notify placeholder) */

    // HyDE [Utilities|Screen Capture]
    KEYSECTION("󰹑", "Capture")
    { MODKEY,                           XK_p,       spawn,          SHCMD("dwm-screenshot gui"), "region → file + clipboard" },    /* HyDE Super+P snip */
    { MODKEY|ControlMask,               XK_p,       spawn,          SHCMD("dwm-screenshot clip"), "region → clipboard" },   /* HyDE Super+Ctrl+P freeze+snip -> box clip */
    { MODKEY|Mod1Mask,                  XK_p,       spawn,          SHCMD("dwm-screenshot screen"), "monitor → file + clipboard" }, /* HyDE Super+Alt+P print monitor */
    { 0,                                XK_Print,   spawn,          SHCMD("dwm-screenshot full"), "all monitors → file + clipboard" },   /* HyDE Print: all monitors */
    { MODKEY,                           XK_u,       spawn,          SHCMD("maim --select | xclip -selection clipboard -t image/png"), "region → clipboard (maim)" }, /* chadwm-native */
    { MODKEY|ControlMask,               XK_u,       spawn,          SHCMD("maim | xclip -selection clipboard -t image/png"), "full screen → clipboard" },        /* chadwm-native */
    { MODKEY,                           XK_o,       spawn,          SHCMD("~/.config/chadwm/scripts/ocr.sh"), "OCR region → clipboard" }, /* HyDE Super+O OCR */
    { MODKEY|ControlMask,               XK_s,       spawn,          SHCMD("~/.config/chadwm/scripts/ocr.sh"), "OCR region → clipboard" }, /* HyDE Super+Ctrl+S OCR */
    { MODKEY|ShiftMask,                 XK_p,       spawn,          SHCMD("~/.config/chadwm/scripts/colorpick.sh"), "colour picker → clipboard" }, /* HyDE Super+Shift+P color picker */

    // HyDE [Hardware Controls]: audio / media / brightness
    KEYSECTION("󰕾", "Media")
    {0,             XF86XK_AudioRaiseVolume,    spawn, {.v = upvol},   "volume up 5%" },
    {0,             XK_F12,                     spawn, {.v = upvol},   "volume up 5%" },    /* HyDE F12 volume up */
    {0,             XF86XK_AudioLowerVolume,    spawn, {.v = downvol}, "volume down 5%" },
    {0,             XK_F11,                     spawn, {.v = downvol}, "volume down 5%" },  /* HyDE F11 volume down */
    {0,             XF86XK_AudioMute,           spawn, {.v = mutevol}, "mute output" },
    {0,             XK_F10,                     spawn, {.v = mutevol}, "mute output" },  /* HyDE F10 toggle mute */
    {0,             XF86XK_AudioMicMute,        spawn, SHCMD("pactl set-source-mute @DEFAULT_SOURCE@ toggle"), "mute microphone" },
    {0,             XK_F9,                      spawn, SHCMD("playerctl pause && pactl set-sink-mute 0 1"), "pause player + mute" }, /* HyDE F9 */
    {0,             XF86XK_AudioPlay,           spawn, SHCMD("playerctl play-pause"), "play / pause" },
    {0,             XF86XK_AudioPause,          spawn, SHCMD("playerctl play-pause"), "play / pause" },
    {0,             XF86XK_AudioPrev,           spawn, SHCMD("playerctl previous"), "previous / next track" },
    {0,             XF86XK_AudioNext,           spawn, SHCMD("playerctl next"), "previous / next track" },
    {MODKEY,        XK_F1,                      spawn, SHCMD("notify-send \"$(playerctl metadata --format '{{artist}} - {{title}}')\""), "now playing" }, /* HyDE songid */
    {MODKEY|ControlMask, XK_m,                  spawn, SHCMD("active-audio mute"), "mute the focused app" }, /* HyDE Super+Ctrl+M mute active window */

    // HyDE [Workspaces] -- dwm tags 1..10
    KEYSECTION("󱇙", "Workspaces")
    TAGKEYS(                            XK_1,                       0)
    TAGKEYS(                            XK_2,                       1)
    TAGKEYS(                            XK_3,                       2)
    TAGKEYS(                            XK_4,                       3)
    TAGKEYS(                            XK_5,                       4)
    TAGKEYS(                            XK_6,                       5)
    TAGKEYS(                            XK_7,                       6)
    TAGKEYS(                            XK_8,                       7)
    TAGKEYS(                            XK_9,                       8)
    TAGKEYS(                            XK_0,                       9) /* HyDE workspace 10 */
    SILENTTAG(                          XK_1,                       0)
    SILENTTAG(                          XK_2,                       1)
    SILENTTAG(                          XK_3,                       2)
    SILENTTAG(                          XK_4,                       3)
    SILENTTAG(                          XK_5,                       4)
    SILENTTAG(                          XK_6,                       5)
    SILENTTAG(                          XK_7,                       6)
    SILENTTAG(                          XK_8,                       7)
    SILENTTAG(                          XK_9,                       8)
    SILENTTAG(                          XK_0,                       9)
    { MODKEY|ControlMask,               XK_Left,    shiftview,      {.i = -1},      "previous / next workspace" },
    { MODKEY|ControlMask,               XK_Right,   shiftview,      {.i = +1},      "previous / next workspace" }, /* HyDE relative workspace forward (verified live 2026-09-20) */
    { MODKEY|ControlMask,               XK_Down,    viewempty,      {0},            "first empty workspace" },   /* HyDE nearest empty workspace */
    { MODKEY|ControlMask|Mod1Mask,      XK_Left,    tagrel,         {.i = -1},      "send window prev / next" }, /* HyDE move window to prev workspace */
    { MODKEY|ControlMask|Mod1Mask,      XK_Right,   tagrel,         {.i = +1},      "send window prev / next" }, /* HyDE move window to next workspace */
    { MODKEY,                           XK_s,       togglescratch,  {0},            "special workspace" },   /* HyDE Super+S special workspace toggle */
    { MODKEY|ShiftMask,                 XK_s,       scratchsend,    {.i = 1},       "window → special, follow" }, /* HyDE Super+Shift+S send to special workspace */
    { MODKEY|Mod1Mask,                  XK_s,       scratchsend,    {.i = 0},       "window → special, silent" }, /* HyDE Super+Alt+S send silently */

    // chadwm-native layout / gap / border controls (keys free in the HyDE map)
    KEYSECTION("󰕴", "Layout")
    { MODKEY,                           XK_space,   setlayout,      {0},            "previous layout" },
    { MODKEY|ControlMask,               XK_comma,   cyclelayout,    {.i = -1},      "cycle layouts" },
    { MODKEY|ControlMask,               XK_period,  cyclelayout,    {.i = +1},      "cycle layouts" },
    { MODKEY,                           XK_m,       setlayout,      {.v = &layouts[2]}, "monocle" },
    { MODKEY|ControlMask,               XK_g,       setlayout,      {.v = &layouts[11]}, "gapless grid" },
    { MODKEY|ControlMask|ShiftMask,     XK_t,       setlayout,      {.v = &layouts[14]}, "floating (no tiling)" },
    { MODKEY,                           XK_i,       incnmaster,     {.i = +1},      "one more master window" },
    { MODKEY|Mod1Mask,                  XK_i,       incnmaster,     {.i = -1},      "one fewer master window" }, /* was Super+D (taken by HyDE dictation) */
    { MODKEY|ShiftMask,                 XK_b,       togglebar,      {0},            "toggle bar" },  /* HyDE Super+Shift+B waybar toggle */

    KEYSECTION("󰃎", "Gaps & borders")
    { MODKEY|ControlMask,               XK_i,       incrgaps,       {.i = +1 },     "all gaps +1" },
    { MODKEY|ControlMask,               XK_d,       incrgaps,       {.i = -1 },     "all gaps −1" },
    { MODKEY|ShiftMask,                 XK_i,       incrigaps,      {.i = +1 },     "inner gaps +1" },
    { MODKEY|ControlMask|ShiftMask,     XK_i,       incrigaps,      {.i = -1 },     "inner gaps −1" },
    { MODKEY|ControlMask,               XK_o,       incrogaps,      {.i = +1 },     "outer gaps +1" },
    { MODKEY|ControlMask|ShiftMask,     XK_o,       incrogaps,      {.i = -1 },     "outer gaps −1" },
    /* inner/outer hori, vert gap trims — on Super+Ctrl+Alt+6..9 so Super+Alt+6..9
       stays free for HyDE silent workspace moves */
    { MODKEY|ControlMask|Mod1Mask,      XK_6,       incrihgaps,     {.i = +1 },     "gap +1 (ih iv oh ov)" },
    { MODKEY|ControlMask|Mod1Mask,      XK_7,       incrivgaps,     {.i = +1 },     "gap +1 (ih iv oh ov)" },
    { MODKEY|ControlMask|Mod1Mask,      XK_8,       incrohgaps,     {.i = +1 },     "gap +1 (ih iv oh ov)" },
    { MODKEY|ControlMask|Mod1Mask,      XK_9,       incrovgaps,     {.i = +1 },     "gap +1 (ih iv oh ov)" },
    { MODKEY|ControlMask|Mod1Mask|ShiftMask, XK_6,  incrihgaps,     {.i = -1 },     "gap −1 (ih iv oh ov)" },
    { MODKEY|ControlMask|Mod1Mask|ShiftMask, XK_7,  incrivgaps,     {.i = -1 },     "gap −1 (ih iv oh ov)" },
    { MODKEY|ControlMask|Mod1Mask|ShiftMask, XK_8,  incrohgaps,     {.i = -1 },     "gap −1 (ih iv oh ov)" },
    { MODKEY|ControlMask|Mod1Mask|ShiftMask, XK_9,  incrovgaps,     {.i = -1 },     "gap −1 (ih iv oh ov)" },
    { MODKEY|ControlMask|ShiftMask,     XK_d,       defaultgaps,    {0},            "reset gaps" },
    { MODKEY|ControlMask,               XK_t,       togglegaps,     {0},            "toggle gaps" },
    { MODKEY|Mod1Mask,                  XK_equal,   setborderpx,    {.i = +1},      "border +1" },
    { MODKEY|ShiftMask,                 XK_minus,   setborderpx,    {.i = -1},      "border −1" },
    { MODKEY|Mod1Mask|ShiftMask,        XK_0,       setborderpx,    {.i = 0},       "reset border" }, /* reset to default_border (Super+Alt+0 = HyDE silent move) */

    // HyDE [Theming and Wallpaper]
    KEYSECTION("󰏘", "Theme")
    { MODKEY|ShiftMask,                 XK_w,       spawn,          SHCMD("~/.config/chadwm/scripts/wallpaper-select.sh"), "wallpaper planet picker" }, /* HyDE Super+Shift+W global wallpaper */
    { MODKEY|ShiftMask,                 XK_t,       spawn,          SHCMD("~/.config/chadwm/scripts/theme-select.sh"), "theme picker" },     /* HyDE Super+Shift+T theme select */
    { MODKEY|Mod1Mask,                  XK_Up,      spawn,          SHCMD("~/.config/chadwm/scripts/bar-theme.sh prev"), "previous / next bar theme" },   /* HyDE Super+Alt+Up bar layout prev */
    { MODKEY|Mod1Mask,                  XK_Down,    spawn,          SHCMD("~/.config/chadwm/scripts/bar-theme.sh next"), "previous / next bar theme" },   /* HyDE Super+Alt+Down bar layout next */
    { MODKEY|ShiftMask,                 XK_y,       spawn,          SHCMD("notify-send 'Animations: not available on X11'"), "animations (none on X11)" }, /* HyDE Super+Shift+Y (no twin) */
    { MODKEY|ShiftMask,                 XK_u,       spawn,          SHCMD("notify-send 'Locker layouts: not available on X11'"), "locker layouts (none on X11)" }, /* HyDE Super+Shift+U (no twin) */

    // HyDE [Window Management]: session
    KEYSECTION("󰐥", "Session")
    { MODKEY,                           XK_l,       spawn,          SHCMD("dwm-lock"), "lock screen" }, /* HyDE Super+L lock */
    { Mod1Mask|ControlMask,             XK_BackSpace, spawn,        SHCMD("chadpower || ~/.config/chadwm/scripts/powermenu.sh"), "session / power menu" }, /* chadpower (Rust popup) */
    { Mod1Mask|ControlMask,             XK_Delete,  spawn,          SHCMD("chadpower || ~/.config/chadwm/scripts/powermenu.sh"), "session / power menu" }, /* HyDE Ctrl+Alt+Del logout menu */
    { MODKEY|ShiftMask,                 XK_r,       restart,        {0},            "restart chadwm" },   /* chadwm-native restart (HyDE Super+Shift+R wallbash: no twin) */
    { MODKEY,                           XK_Delete,  spawn,          SHCMD("killall bar.sh chadwm"), "end session" }, /* HyDE Super+Delete kill session */
    { MODKEY|ControlMask,               XK_q,       spawn,          SHCMD("killall bar.sh chadwm"), "end session" }, /* chadwm-native quit */
};

/* button definitions */
/* click can be ClkTagBar, ClkLtSymbol, ClkStatusText, ClkWinTitle, ClkClientWin, or ClkRootWin */
static const Button buttons[] = {
    /* click                event mask      button          function        argument        description */
    { ClkLtSymbol,          0,              Button1,        setlayout,      {0},            "previous layout" },
    { ClkLtSymbol,          0,              Button3,        setlayout,      {.v = &layouts[3]}, "spiral layout" },
    { ClkWinTitle,          0,              Button2,        zoom,           {0},            "make master" },
    { ClkStatusText,        0,              Button2,        spawn,          SHCMD("st"),    "terminal (st)" },

    /* HyDE Super+scroll = prev/next workspace (scroll up = previous) */
    { ClkRootWin,           0,              Button4,        shiftview,      {.i = -1},      "previous / next workspace" },
    { ClkRootWin,           0,              Button5,        shiftview,      {.i = +1},      "previous / next workspace" },
    { ClkTagBar,            0,              Button4,        shiftview,      {.i = -1},      "previous / next workspace" },
    { ClkTagBar,            0,              Button5,        shiftview,      {.i = +1},      "previous / next workspace" },
    /* same with Super held, 1:1 with the HyDE map (MOD + mouse_down/up) */
    { ClkRootWin,           MODKEY,         Button4,        shiftview,      {.i = -1},      "previous / next workspace" },
    { ClkRootWin,           MODKEY,         Button5,        shiftview,      {.i = +1},      "previous / next workspace" },
    { ClkTagBar,            MODKEY,         Button4,        shiftview,      {.i = -1},      "previous / next workspace" },
    { ClkTagBar,            MODKEY,         Button5,        shiftview,      {.i = +1},      "previous / next workspace" },

    /* Keep movemouse? */
    /* { ClkClientWin,         MODKEY,         Button1,        movemouse,      {0} }, */

    /* placemouse options, choose which feels more natural:
    *    0 - tiled position is relative to mouse cursor
    *    1 - tiled position is relative to window center
    *    2 - mouse pointer warps to window center
    *
    * The moveorplace uses movemouse or placemouse depending on the floating state
    * of the selected client. Set up individual keybindings for the two if you want
    * to control these separately (i.e. to retain the feature to move a tiled window
    * into a floating position).
    */
    { ClkClientWin,         MODKEY,         Button1,        moveorplace,    {.i = 0},       "drag to move" },
    { ClkClientWin,         MODKEY,         Button2,        togglefloating, {0},            "toggle floating" },
    { ClkClientWin,         MODKEY,         Button3,        resizemouse,    {0},            "drag corner to resize" },
    { ClkClientWin,         MODKEY|ControlMask, Button1,    dragmfact,      {0},            "drag master area size" },
    { ClkClientWin,         MODKEY|ControlMask, Button3,    dragcfact,      {0},            "drag window size factor" },
    { ClkTagBar,            0,              Button1,        view,           {0},            "go to workspace" },
    { ClkTagBar,            0,              Button3,        toggleview,     {0},            "show workspace alongside" },
    { ClkTagBar,            MODKEY,         Button1,        tag,            {0},            "move window there" },
    { ClkTagBar,            MODKEY,         Button3,        toggletag,      {0},            "also show window there" },
    { ClkTabBar,            0,              Button1,        focuswin,       {0},            "focus tab" },
    { ClkTabPrev,           0,              Button1,        movestack,      { .i = -1 },    "move tab left" },
    { ClkTabNext,           0,              Button1,        movestack,      { .i = +1 },    "move tab right" },
    { ClkTabClose,          0,              Button1,        killclient,     {0},            "close tab" },
};
