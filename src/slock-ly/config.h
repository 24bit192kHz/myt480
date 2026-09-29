/* slock-ly config */

/* fonts (fontconfig names; pixel sizes are derived from screen height) */
static const char *font_clock = "Adwaita Sans:style=Light";
static const char *font_text  = "Adwaita Sans";
static const char *font_icon  = "FiraCode Nerd Font";   /* battery + power glyphs */

/* percent of screen height */
static const int clock_pct = 13;     /* HH:MM */
static const int date_pct  = 25;     /* tenths of a percent: 2.5% */
static const int text_pct  = 17;     /* tenths: status + dots */
static const int bar_pct   = 16;     /* tenths: bottom bar */

/* translucent black veil over the desktop, 0..0xffff (0x8000 = 50%) */
static const unsigned short veil_alpha = 0x8000;   /* 50% */

static const char *col_fg     = "#e8e8ec";
static const char *col_dim    = "#9a9aa6";
static const char *col_err    = "#ff8080";
static const char *col_ok     = "#9fe0a8";
static const char *col_accent = "#8ab4f8";

/* how long an error message stays up (seconds) */
static const int err_secs = 3;

/* power row: second click within this many seconds confirms */
static const int confirm_secs = 3;
static const char *cmd_poweroff = "/usr/bin/loginctl poweroff";
static const char *cmd_reboot   = "/usr/bin/loginctl reboot";
static const char *cmd_suspend  = "/usr/bin/loginctl suspend";
