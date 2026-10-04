/* keyhelp.c — the Super+? cheat sheet is generated from keys[] / buttons[]
 * (2026-10-01), so it lists exactly what this binary binds.
 *
 * dumpkeys() writes one TSV row per entry:
 *   S <tab> icon <tab> section              KEYSECTION header
 *   K <tab> mods <tab> keysym <tab> desc <tab> N    key binding keys[N]
 *   B <tab> mods <tab> button <tab> click <tab> desc
 * mods is "Super+Ctrl+Alt+Shift" (any subset, "" for none), keysym is
 * XKeysymToString().
 * `chadwm -k` prints it; keyhelp() pipes it into scripts/keyhelp.sh, which
 * merges, formats and shows it in rofi. Enter in the sheet runs
 * `chadwm -x N`: a _CHADWM_KEY root message makes the WM call keys[N]
 * itself, so it works whatever the keyboard layout (xdotool sends the wrong
 * keycode for "/" under ara,us). */

static const char *clicknames[] = {
	[ClkTagBar] = "tags", [ClkTabBar] = "tab", [ClkTabPrev] = "tab-prev",
	[ClkTabNext] = "tab-next", [ClkTabClose] = "tab-close",
	[ClkLtSymbol] = "layout", [ClkStatusText] = "status",
	[ClkWinTitle] = "title", [ClkClientWin] = "window", [ClkRootWin] = "desktop",
};

static void
modnames(char *buf, size_t len, unsigned int mod)
{
	static const struct { unsigned int mask; const char *name; } m[] = {
		{ Mod4Mask, "Super" }, { ControlMask, "Ctrl" },
		{ Mod1Mask, "Alt" }, { ShiftMask, "Shift" },
	};
	size_t i, n = 0;

	buf[0] = '\0';
	for (i = 0; i < LENGTH(m) && n < len; i++)
		if (mod & m[i].mask)
			n += snprintf(buf + n, len - n, "%s%s", n ? "+" : "", m[i].name);
}

static void
dumpkeys(FILE *f)
{
	unsigned int i;
	const char *ks;
	char mods[32];

	for (i = 0; i < LENGTH(keys); i++) {
		if (!keys[i].func) {
			fprintf(f, "S\t%s\n", keys[i].desc ? keys[i].desc : "\t");
			continue;
		}
		modnames(mods, sizeof mods, keys[i].mod);
		ks = XKeysymToString(keys[i].keysym);
		fprintf(f, "K\t%s\t%s\t%s\t%u\n", mods, ks ? ks : "?",
		        keys[i].desc ? keys[i].desc : "", i);
	}
	for (i = 0; i < LENGTH(buttons); i++) {
		modnames(mods, sizeof mods, buttons[i].mask);
		fprintf(f, "B\t%s\t%u\t%s\t%s\n", mods, buttons[i].button,
		        buttons[i].click < LENGTH(clicknames) && clicknames[buttons[i].click]
		        ? clicknames[buttons[i].click] : "?",
		        buttons[i].desc ? buttons[i].desc : "");
	}
}

/* arg->v: shell command that reads the dump on stdin. The writer is a
 * forked copy of the WM, so the sheet shows the running binary's bindings
 * and the WM never waits on the pipe. */
void
keyhelp(const Arg *arg)
{
	struct sigaction sa;
	FILE *p;

	if (fork() != 0)
		return;
	if (dpy)
		close(ConnectionNumber(dpy));
	setsid();
	sigemptyset(&sa.sa_mask);
	sa.sa_flags = 0;
	sa.sa_handler = SIG_DFL;
	sigaction(SIGCHLD, &sa, NULL);
	if (!(p = popen((const char *)arg->v, "w")))
		_exit(1);
	dumpkeys(p);
	_exit(pclose(p) == 0 ? 0 : 1);
}

/* chadwm -x N: ask the running chadwm to run keys[N] */
static int
sendkey(const char *n)
{
	Display *d;
	XEvent ev = { .xclient = { .type = ClientMessage, .format = 32 } };
	char *end;

	ev.xclient.data.l[0] = strtol(n, &end, 10);
	if (*n == '\0' || *end != '\0' || ev.xclient.data.l[0] < 0)
		die("chadwm -x: bad key index '%s'", n);
	if (!(d = XOpenDisplay(NULL)))
		die("chadwm -x: cannot open display");
	ev.xclient.window = DefaultRootWindow(d);
	ev.xclient.message_type = XInternAtom(d, "_CHADWM_KEY", False);
	XSendEvent(d, ev.xclient.window, False,
	           SubstructureRedirectMask | SubstructureNotifyMask, &ev);
	XCloseDisplay(d);
	return EXIT_SUCCESS;
}
