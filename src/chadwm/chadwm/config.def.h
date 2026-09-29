/* See LICENSE file for copyright and license details. */

#include <X11/XF86keysym.h>

/* appearance */
static const unsigned int borderpx  = 2;        /* HyDE border_size 2 */        /* border pixel of windows */
static const unsigned int default_border = 2;   /* to switch back to default border after dynamic border resizing via keybinds */
static const unsigned int snap      = 32;       /* snap pixel */
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
 * immediate bar refresh (SIGUSR1 to bar.sh) */
static const char *upvol[]   = { "/bin/sh", "-c", "exec \"$HOME/.config/chadwm/scripts/vol.sh\" up",   NULL };
static const char *downvol[] = { "/bin/sh", "-c", "exec \"$HOME/.config/chadwm/scripts/vol.sh\" down", NULL };
static const char *mutevol[] = { "/bin/sh", "-c", "exec \"$HOME/.config/chadwm/scripts/vol.sh\" mute", NULL };

static const int new_window_attach_on_end = 1; /* dwindle: older windows keep their place (Hyprland) */ /*  1 means the new window will attach on the end; 0 means the new window will attach on the front,default is front */
#define ICONSIZE 19   /* icon size */
#define ICONSPACING 8 /* space between icon and title */

static const char *fonts[]          = {"JetBrainsMono Nerd Font:style:Medium:size=11" ,"JetBrainsMono Nerd Font Mono:style:medium:size=19" };

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
    { MODKEY,                       KEY,      view,           {.ui = 1 << TAG} }, \
    { MODKEY|ControlMask,           KEY,      toggleview,     {.ui = 1 << TAG} }, \
    { MODKEY|ShiftMask,             KEY,      tag,            {.ui = 1 << TAG} }, \
    { MODKEY|ControlMask|ShiftMask, KEY,      toggletag,      {.ui = 1 << TAG} },
/* HyDE: Super+Alt+N moves the window to workspace N without following it
 * (dwm `tag` changes the client's tags only, so the view stays put) */
#define SILENTTAG(KEY,TAG) \
    { MODKEY|Mod1Mask,              KEY,      tag,            {.ui = 1 << TAG} },

/* helper for spawning shell commands in the pre dwm-5.0 fashion */
#define SHCMD(cmd) { .v = (const char*[]){ "/bin/sh", "-c", cmd, NULL } }

/* Keymap ported 2026-09-20 from the owner's Hyprland map (HyDE,
 * https://github.com/24bit192kHz/HyDE, Configs/.local/share/hypr/lua/key_binds.lua).
 * Full combo list: ~/.config/chadwm/keybinds.txt. Unmappable HyDE binds (pin,
 * center/resize-30 moon scripts, hyprshaderd dimming, hyprlock layouts,
 * Super+Alt+N earth-native) are intentionally dropped. */
static const Key keys[] = {
    /* modifier                         key         function        argument */

    // HyDE [Hardware Controls]: audio / media / brightness
    {0,             XF86XK_AudioLowerVolume,    spawn, {.v = downvol}},
    {0,             XF86XK_AudioMute,           spawn, {.v = mutevol}},
    {0,             XF86XK_AudioRaiseVolume,    spawn, {.v = upvol}},
    {0,             XF86XK_AudioMicMute,        spawn, SHCMD("pactl set-source-mute @DEFAULT_SOURCE@ toggle")},
    {0,             XF86XK_AudioPlay,           spawn, SHCMD("playerctl play-pause")},
    {0,             XF86XK_AudioPause,          spawn, SHCMD("playerctl play-pause")},
    {0,             XF86XK_AudioNext,           spawn, SHCMD("playerctl next")},
    {0,             XF86XK_AudioPrev,           spawn, SHCMD("playerctl previous")},
    {0,             XK_F10,                     spawn, {.v = mutevol}},  /* HyDE F10 toggle mute */
    {0,             XK_F11,                     spawn, {.v = downvol}},  /* HyDE F11 volume down */
    {0,             XK_F12,                     spawn, {.v = upvol}},    /* HyDE F12 volume up */
    {0,             XK_F9,                      spawn, SHCMD("playerctl pause && pactl set-sink-mute 0 1")}, /* HyDE F9 */
    {MODKEY,        XK_F1,                      spawn, SHCMD("notify-send \"$(playerctl metadata --format '{{artist}} - {{title}}')\"")}, /* HyDE songid */
    {MODKEY|ControlMask, XK_m,                  spawn, SHCMD("active-audio mute")}, /* HyDE Super+Ctrl+M mute active window */

    // HyDE [Window Management]
    { MODKEY,                           XK_q,       killclient,     {0} },
    { Mod1Mask,                         XK_F4,      killclient,     {0} },   /* HyDE ALT+F4 */
    { MODKEY,                           XK_Delete,  spawn,        SHCMD("killall bar.sh chadwm") }, /* HyDE Super+Delete kill session */
    { MODKEY,                           XK_w,       togglefloating, {0} },
    { MODKEY,                           XK_g,       tabmode,        {.i = -1} }, /* HyDE group toggle -> chadwm tab bar */
    { ShiftMask,                        XK_F11,     togglefullscr,  {0} },   /* HyDE SHIFT+F11 */
    { MODKEY,                           XK_f,       togglefullscr,  {0} },   /* HyDE Super+F */
    { MODKEY,                           XK_l,       spawn,        SHCMD("dwm-lock") }, /* HyDE Super+L lock */
    { MODKEY|ControlMask,               XK_q,       spawn,        SHCMD("killall bar.sh chadwm") }, /* chadwm-native quit */
    { MODKEY|ShiftMask,                 XK_r,       restart,        {0} },   /* chadwm-native restart (HyDE Super+Shift+R wallbash: no twin) */
    { MODKEY|ShiftMask,                 XK_f,       togglepin,      {0} },   /* HyDE Super+Shift+F pin window */
    { Mod1Mask|ControlMask,             XK_Delete,  spawn,        SHCMD("~/.config/chadwm/scripts/powermenu.sh") }, /* HyDE Ctrl+Alt+Del logout menu */

    // HyDE [Group Navigation] -> tab / stack order
    { MODKEY|ControlMask,               XK_h,       focusstack,     {.i = -1} }, /* HyDE group prev */
    { MODKEY|ControlMask,               XK_l,       focusstack,     {.i = +1} }, /* HyDE group next */

    // HyDE [Change focus] / [Resize] / [Move active window]
    { MODKEY,                           XK_Left,    focusdir,       {.i = 0} }, /* HyDE focus left */
    { MODKEY,                           XK_Right,   focusdir,       {.i = 1} }, /* HyDE focus right */
    { MODKEY,                           XK_Up,      focusdir,       {.i = 2} }, /* HyDE focus up */
    { MODKEY,                           XK_Down,    focusdir,       {.i = 3} }, /* HyDE focus down */
    { Mod1Mask,                         XK_Tab,     focusstack,     {.i = +1} }, /* HyDE ALT+TAB cycle focus */
    { MODKEY,                           XK_Tab,     spawn,        SHCMD("rofi -show window") }, /* HyDE Super+TAB window switcher */
    /* HyDE resizeactive +-30 px (dwindle: the focused window's split) */
    { MODKEY|ShiftMask,                 XK_Left,    resizedir,      {.i = 0} },
    { MODKEY|ShiftMask,                 XK_Right,   resizedir,      {.i = 1} },
    { MODKEY|ShiftMask,                 XK_Up,      resizedir,      {.i = 2} },
    { MODKEY|ShiftMask,                 XK_Down,    resizedir,      {.i = 3} },
    /* HyDE movewindow: swap with the neighbour (floating: move 30 px) */
    { MODKEY|ControlMask|ShiftMask,     XK_Left,    movedir,        {.i = 0} },
    { MODKEY|ControlMask|ShiftMask,     XK_Right,   movedir,        {.i = 1} },
    { MODKEY|ControlMask|ShiftMask,     XK_Up,      movedir,        {.i = 2} },
    { MODKEY|ControlMask|ShiftMask,     XK_Down,    movedir,        {.i = 3} },
    /* HyDE Super+Z / Super+X: hold and move the mouse to drag / resize */
    { MODKEY,                           XK_z,       moveorplace,    {.i = 0} },
    { MODKEY,                           XK_x,       resizemouse,    {0} },

    // HyDE [Launcher|Apps]
    { MODKEY,                           XK_Return,  spawn,          SHCMD("st")},
    { MODKEY,                           XK_t,       spawn,          SHCMD("st")},  /* HyDE Super+T */
    { MODKEY|Mod1Mask,                  XK_t,       droptoggle,     {0} }, /* HyDE dropdown terminal (pypr console) */
    { MODKEY,                           XK_e,       spawn,          SHCMD("pcmanfm-qt")},
    { MODKEY,                           XK_b,       spawn,          SHCMD("firefox")},
    { ControlMask|ShiftMask,            XK_Escape,  spawn,          SHCMD("st -e btop")},

    // HyDE [Launcher|Rofi menus]
    { MODKEY,                           XK_a,       spawn,          SHCMD("rofi -show drun")},
    { MODKEY,                           XK_c,       centerwin,      {0} },   /* HyDE Super+C centre window */
    { MODKEY|ShiftMask,                 XK_c,       resizepct,      {.i = 30} }, /* HyDE Super+Shift+C resize to 30%% */
    { MODKEY|ShiftMask,                 XK_e,       spawn,          SHCMD("rofi -modes filebrowser -show filebrowser")}, /* HyDE file finder */
    { MODKEY,                           XK_slash,   spawn,          SHCMD("rofi -dmenu -i -p keybinds < ~/.config/chadwm/keybinds.txt")}, /* HyDE keybindings hint */
    { MODKEY,                           XK_comma,   spawn,          SHCMD("rofimoji --action type --selector rofi --clipboarder xclip --typer xdotool")}, /* HyDE emoji picker */
    { MODKEY,                           XK_period,  spawn,          SHCMD("~/.config/chadwm/scripts/glyph-picker.sh")}, /* HyDE glyph picker */
    { MODKEY,                           XK_v,       spawn,          SHCMD("rofi-clip.sh pick")},   /* HyDE clipboard */
    { MODKEY|ShiftMask,                 XK_v,       spawn,          SHCMD("rofi-clip.sh menu")},   /* HyDE clipboard manager */
    { MODKEY|ShiftMask,                 XK_a,       spawn,          SHCMD("rofi -show combi")},    /* HyDE select rofi launcher */
    // HyDE [Utilities|Screen Capture]
    { MODKEY,                           XK_u,       spawn,          SHCMD("maim --select | xclip -selection clipboard -t image/png")}, /* chadwm-native */
    { MODKEY|ControlMask,               XK_u,       spawn,          SHCMD("maim | xclip -selection clipboard -t image/png")},        /* chadwm-native */
    { MODKEY,                           XK_p,       spawn,          SHCMD("dwm-screenshot gui")},    /* HyDE Super+P snip */
    { MODKEY|ControlMask,               XK_p,       spawn,          SHCMD("dwm-screenshot clip")},   /* HyDE Super+Ctrl+P freeze+snip -> box clip */
    { MODKEY|Mod1Mask,                  XK_p,       spawn,          SHCMD("dwm-screenshot screen")}, /* HyDE Super+Alt+P print monitor */
    { 0,                                XK_Print,   spawn,          SHCMD("dwm-screenshot full")},   /* HyDE Print: all monitors */
    { MODKEY,                           XK_o,       spawn,          SHCMD("~/.config/chadwm/scripts/ocr.sh")}, /* HyDE Super+O OCR */
    { MODKEY|ControlMask,               XK_s,       spawn,          SHCMD("~/.config/chadwm/scripts/ocr.sh")}, /* HyDE Super+Ctrl+S OCR */
    { MODKEY|ShiftMask,                 XK_p,       spawn,          SHCMD("~/.config/chadwm/scripts/colorpick.sh")}, /* HyDE Super+Shift+P color picker */

    // HyDE [Utilities]
    { MODKEY,                           XK_k,       spawn,          SHCMD("kbd-lang.sh toggle")},    /* HyDE Super+K keyboard layout */
    { MODKEY|Mod1Mask,                  XK_g,       spawn,          SHCMD("notify-send 'GameMode on'")}, /* HyDE Super+Alt+G (box: notify placeholder) */
    { MODKEY,                           XK_d,       spawn,          SHCMD("notify-send Dictation 'no STT backend installed'")}, /* HyDE Super+D dictation (box: no STT) */
    { MODKEY,                           XK_s,       togglescratch,  {0} },   /* HyDE Super+S special workspace toggle */
    { MODKEY|ShiftMask,                 XK_s,       scratchsend,    {.i = 1} }, /* HyDE Super+Shift+S send to special workspace */
    { MODKEY|Mod1Mask,                  XK_s,       scratchsend,    {.i = 0} }, /* HyDE Super+Alt+S send silently */

    // HyDE [Theming and Wallpaper]
    { MODKEY|ShiftMask,                 XK_w,       spawn,          SHCMD("~/.config/chadwm/scripts/wallpaper-select.sh")}, /* HyDE Super+Shift+W global wallpaper */
    { MODKEY|Mod1Mask,                  XK_Up,      spawn,          SHCMD("~/.config/chadwm/scripts/bar-theme.sh prev")},   /* HyDE Super+Alt+Up bar layout prev */
    { MODKEY|Mod1Mask,                  XK_Down,    spawn,          SHCMD("~/.config/chadwm/scripts/bar-theme.sh next")},   /* HyDE Super+Alt+Down bar layout next */
    { MODKEY|ShiftMask,                 XK_t,       spawn,          SHCMD("~/.config/chadwm/scripts/theme-select.sh")},     /* HyDE Super+Shift+T theme select */
    { MODKEY|ShiftMask,                 XK_y,       spawn,          SHCMD("notify-send 'Animations: not available on X11'")}, /* HyDE Super+Shift+Y (no twin) */
    { MODKEY|ShiftMask,                 XK_u,       spawn,          SHCMD("notify-send 'Locker layouts: not available on X11'")}, /* HyDE Super+Shift+U (no twin) */

    // chadwm-native layout / gap / border controls (keys free in the HyDE map)
    { MODKEY,                           XK_j,       togglesplit,    {0} },   /* HyDE Super+J toggle split */
    { MODKEY,                           XK_space,   setlayout,      {0} },
    { MODKEY|ShiftMask,                 XK_space,   togglefloating, {0} },
    { MODKEY,                           XK_m,       setlayout,      {.v = &layouts[2]} }, /* monocle */
    { MODKEY|ControlMask,               XK_g,       setlayout,      {.v = &layouts[11]} },
    { MODKEY|ControlMask|ShiftMask,     XK_t,       setlayout,      {.v = &layouts[14]} },
    { MODKEY|ControlMask,               XK_comma,   cyclelayout,    {.i = -1} },
    { MODKEY|ControlMask,               XK_period,  cyclelayout,    {.i = +1} },
    { MODKEY,                           XK_i,       incnmaster,     {.i = +1} },
    { MODKEY|Mod1Mask,                  XK_i,       incnmaster,     {.i = -1} }, /* was Super+D (taken by HyDE dictation) */
    { MODKEY|ShiftMask,                 XK_o,       setcfact,       {.f =  0.00} },
    { MODKEY|ControlMask,               XK_i,       incrgaps,       {.i = +1 } },
    { MODKEY|ControlMask,               XK_d,       incrgaps,       {.i = -1 } },
    { MODKEY|ShiftMask,                 XK_i,       incrigaps,      {.i = +1 } },
    { MODKEY|ControlMask|ShiftMask,     XK_i,       incrigaps,      {.i = -1 } },
    { MODKEY|ControlMask,               XK_o,       incrogaps,      {.i = +1 } },
    { MODKEY|ControlMask|ShiftMask,     XK_o,       incrogaps,      {.i = -1 } },
    { MODKEY|ControlMask|ShiftMask,     XK_d,       defaultgaps,    {0} },
    /* inner/outer hori, vert gap trims — on Super+Ctrl+Alt+6..9 so Super+Alt+6..9
       stays free for HyDE silent workspace moves */
    { MODKEY|ControlMask|Mod1Mask,      XK_6,       incrihgaps,     {.i = +1 } },
    { MODKEY|ControlMask|Mod1Mask|ShiftMask, XK_6,  incrihgaps,     {.i = -1 } },
    { MODKEY|ControlMask|Mod1Mask,      XK_7,       incrivgaps,     {.i = +1 } },
    { MODKEY|ControlMask|Mod1Mask|ShiftMask, XK_7,  incrivgaps,     {.i = -1 } },
    { MODKEY|ControlMask|Mod1Mask,      XK_8,       incrohgaps,     {.i = +1 } },
    { MODKEY|ControlMask|Mod1Mask|ShiftMask, XK_8,  incrohgaps,     {.i = -1 } },
    { MODKEY|ControlMask|Mod1Mask,      XK_9,       incrovgaps,     {.i = +1 } },
    { MODKEY|ControlMask|Mod1Mask|ShiftMask, XK_9,  incrovgaps,     {.i = -1 } },
    { MODKEY|ControlMask,               XK_t,       togglegaps,     {0} },
    { MODKEY|ShiftMask,                 XK_b,       togglebar,      {0} },  /* HyDE Super+Shift+B waybar toggle */
    { MODKEY|ShiftMask,                 XK_minus,   setborderpx,    {.i = -1} },
    { MODKEY|Mod1Mask,                  XK_equal,   setborderpx,    {.i = +1} },
    { MODKEY|Mod1Mask|ShiftMask,        XK_0,       setborderpx,    {.i = 0} /* reset to default_border (Super+Alt+0 = HyDE silent move) */ },

    // HyDE [Workspaces] -- dwm tags 1..9
    TAGKEYS(                            XK_1,                       0)
    TAGKEYS(                            XK_2,                       1)
    TAGKEYS(                            XK_3,                       2)
    TAGKEYS(                            XK_4,                       3)
    TAGKEYS(                            XK_5,                       4)
    TAGKEYS(                            XK_6,                       5)
    TAGKEYS(                            XK_7,                       6)
    TAGKEYS(                            XK_8,                       7)
    TAGKEYS(                            XK_9,                       8)
    SILENTTAG(                          XK_1,                       0)
    SILENTTAG(                          XK_2,                       1)
    SILENTTAG(                          XK_3,                       2)
    SILENTTAG(                          XK_4,                       3)
    SILENTTAG(                          XK_5,                       4)
    SILENTTAG(                          XK_6,                       5)
    SILENTTAG(                          XK_7,                       6)
    SILENTTAG(                          XK_8,                       7)
    SILENTTAG(                          XK_9,                       8)
    TAGKEYS(                            XK_0,                       9) /* HyDE workspace 10 */
    SILENTTAG(                          XK_0,                       9)
    { MODKEY|ControlMask,               XK_Right,   shiftview,      {.i = +1} }, /* HyDE relative workspace forward (verified live 2026-09-20) */
    { MODKEY|ControlMask,               XK_Left,    shiftview,      {.i = -1} },
    { MODKEY|ControlMask,               XK_Down,    viewempty,      {0} },   /* HyDE nearest empty workspace */
    { MODKEY|ControlMask|Mod1Mask,      XK_Left,    tagrel,         {.i = -1} }, /* HyDE move window to prev workspace */
    { MODKEY|ControlMask|Mod1Mask,      XK_Right,   tagrel,         {.i = +1} }, /* HyDE move window to next workspace */
};

/* button definitions */
/* click can be ClkTagBar, ClkLtSymbol, ClkStatusText, ClkWinTitle, ClkClientWin, or ClkRootWin */
static const Button buttons[] = {
    /* click                event mask      button          function        argument */
    { ClkLtSymbol,          0,              Button1,        setlayout,      {0} },
    { ClkLtSymbol,          0,              Button3,        setlayout,      {.v = &layouts[3]} },
    { ClkWinTitle,          0,              Button2,        zoom,           {0} },
    { ClkStatusText,        0,              Button2,        spawn,          SHCMD("st") },

    /* HyDE Super+scroll = prev/next workspace (scroll up = previous) */
    { ClkRootWin,           0,              Button4,        shiftview,      {.i = -1} },
    { ClkRootWin,           0,              Button5,        shiftview,      {.i = +1} },
    { ClkTagBar,            0,              Button4,        shiftview,      {.i = -1} },
    { ClkTagBar,            0,              Button5,        shiftview,      {.i = +1} },
    /* same with Super held, 1:1 with the HyDE map (MOD + mouse_down/up) */
    { ClkRootWin,           MODKEY,         Button4,        shiftview,      {.i = -1} },
    { ClkRootWin,           MODKEY,         Button5,        shiftview,      {.i = +1} },
    { ClkTagBar,            MODKEY,         Button4,        shiftview,      {.i = -1} },
    { ClkTagBar,            MODKEY,         Button5,        shiftview,      {.i = +1} },

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
    { ClkClientWin,         MODKEY,         Button1,        moveorplace,    {.i = 0} },
    { ClkClientWin,         MODKEY,         Button2,        togglefloating, {0} },
    { ClkClientWin,         MODKEY,         Button3,        resizemouse,    {0} },
    { ClkClientWin,         ControlMask,    Button1,        dragmfact,      {0} },
    { ClkClientWin,         ControlMask,    Button3,        dragcfact,      {0} },
    { ClkTagBar,            0,              Button1,        view,           {0} },
    { ClkTagBar,            0,              Button3,        toggleview,     {0} },
    { ClkTagBar,            MODKEY,         Button1,        tag,            {0} },
    { ClkTagBar,            MODKEY,         Button3,        toggletag,      {0} },
    { ClkTabBar,            0,              Button1,        focuswin,       {0} },
    { ClkTabPrev,           0,              Button1,        movestack,      { .i = -1 } },
    { ClkTabNext,           0,              Button1,        movestack,      { .i = +1 } },
    { ClkTabClose,          0,              Button1,        killclient,     {0} },
};
