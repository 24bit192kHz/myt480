/* bartip: hover popups for the clickable status modules (^kN^ in bar.sh).
 *
 * When the pointer settles on module N, chadwm writes "SEQ N" to
 * $XDG_RUNTIME_DIR/chadwm-hover and spawns scripts/barhover.sh SEQ N. The
 * script waits a moment, then sets the root property _CHADWM_TIP (UTF-8) to
 * "SEQ\nline\nline..." and, for live modules, refreshes it while the file
 * still says SEQ. A tip whose SEQ is stale is ignored, so a slow script can
 * never show the wrong popup. Lines may carry colour codes (see tipline).
 * Leaving the module, leaving the bar or clicking hides it; after a click
 * the tip stays hidden until the pointer leaves the module.
 */
#define TIPPAD 10
#define TIPMAXLINES 40

static Window tipwin;
static Pixmap tippix;
static Atom tipatom;
static unsigned int tipseq;
static int tipid, tipmuted, tipx, tipy;
static Monitor *tipmon;

static void
tipstate(void)
{
	char path[512];
	const char *dir = getenv("XDG_RUNTIME_DIR");
	FILE *f;

	snprintf(path, sizeof path, "%s/chadwm-hover", dir ? dir : "/tmp");
	if ((f = fopen(path, "w"))) {
		fprintf(f, "%u %d\n", tipseq, tipid);
		fclose(f);
	}
}

static void
tiphide(void)
{
	if (tipwin)
		XUnmapWindow(dpy, tipwin);
}

/* id 0 = nothing hovered; x is the module's left edge in root coords */
static void
tiphover(Monitor *m, int id, int x)
{
	char seq[12], ids[12];

	if (id != tipmuted)
		tipmuted = 0;
	if (tipmuted)
		id = 0;
	if (id == tipid)
		return;
	tiphide();
	tipid = id;
	tipseq++;
	tipmon = m;
	tipx = x;
	tipy = m->by + bh + m->gappoh / 2;
	tipstate();
	if (!id)
		return;
	snprintf(seq, sizeof seq, "%u", tipseq);
	snprintf(ids, sizeof ids, "%d", id);
	spawn(&(Arg){ .v = (const char *[]){ "/bin/sh", "-c",
	      "exec \"$HOME/.config/chadwm/scripts/barhover.sh\" \"$0\" \"$1\"",
	      seq, ids, NULL } });
}

/* a click: hide, and keep this module quiet until the pointer leaves it */
static void
tipclick(void)
{
	int id = tipid;

	tiphover(selmon, 0, 0);
	tipmuted = id;
}

/* draw one line at y (or only measure it). Codes: ^c#rrggbb^ text colour,
 * ^b#rrggbb^ background, ^d^ defaults, ^>N^ the next text ends at column N
 * (columns are the width of "0", for calendar grids) */
static int
tipline(char *s, int y, int draw)
{
	int x = TIPPAD, w, ralign = -1, cw = drw_fontset_getwidth(drw, "0");
	char *p, save;

	drw->scheme[ColFg] = scheme[SchemeNorm][ColFg];
	drw->scheme[ColBg] = scheme[SchemeNorm][ColBg];
	while (*s) {
		for (p = s; *p && *p != '^'; p++)
			;
		save = *p;
		*p = '\0';
		if (*s) {
			w = drw_fontset_getwidth(drw, s);
			if (ralign >= 0 && TIPPAD + ralign * cw - w > x)
				x = TIPPAD + ralign * cw - w;
			ralign = -1;
			if (draw) {
				drw_text(drw, 0, 0, w, drw->fonts->h, 0, s, 0);
				XCopyArea(dpy, drw->drawable, tippix, drw->gc, 0, 0, w,
				          drw->fonts->h, x, y);
			}
			x += w;
		}
		*p = save;
		if (!*p)
			break;
		s = p + 1;
		if ((p = strchr(s, '^')) == NULL)
			break;
		if ((*s == 'c' || *s == 'b') && p - s == 8) {
			char col[8];
			memcpy(col, s + 1, 7);
			col[7] = '\0';
			drw_clr_create(drw, &drw->scheme[*s == 'c' ? ColFg : ColBg], col);
		} else if (*s == 'd') {
			drw->scheme[ColFg] = scheme[SchemeNorm][ColFg];
			drw->scheme[ColBg] = scheme[SchemeNorm][ColBg];
		} else if (*s == '>') {
			ralign = atoi(s + 1);
		}
		s = p + 1;
	}
	return x + TIPPAD;
}

static void
tipshow(char *text)
{
	char *line[TIPMAXLINES];
	int n = 0, i, w = 0, h, x, lh = drw->fonts->h + 2;
	Monitor *m = tipmon ? tipmon : selmon;
	char *p;

	for (p = text; p && n < TIPMAXLINES; ) {
		line[n++] = p;
		if ((p = strchr(p, '\n')))
			*p++ = '\0';
	}
	while (n > 0 && !*line[n - 1])
		n--;
	if (!n) {
		tiphide();
		return;
	}
	drw_setscheme(drw, scheme[LENGTH(colors)]);
	for (i = 0; i < n; i++)
		if ((x = tipline(line[i], 0, 0)) > w)
			w = x;
	if (w > m->mw - 2 * (int)m->gappov)
		w = m->mw - 2 * m->gappov;
	h = n * lh + 2 * TIPPAD - 2;

	/* left edge under the module, kept on the monitor */
	x = tipx;
	if (x + w + 2 > m->mx + m->mw - (int)m->gappov)
		x = m->mx + m->mw - m->gappov - w - 2;
	if (x < m->mx + (int)m->gappov)
		x = m->mx + m->gappov;

	if (!tipwin) {
		XSetWindowAttributes wa = { .override_redirect = True,
		                            .border_pixel = scheme[SchemeSel][ColBorder].pixel };
		XClassHint ch = { "chadwm-tip", "chadwm-tip" };
		tipwin = XCreateWindow(dpy, root, x, tipy, w, h, 1,
		                       DefaultDepth(dpy, screen), CopyFromParent,
		                       DefaultVisual(dpy, screen),
		                       CWOverrideRedirect | CWBorderPixel, &wa);
		XSetClassHint(dpy, tipwin, &ch);
	}
	if (tippix)
		XFreePixmap(dpy, tippix);
	tippix = XCreatePixmap(dpy, root, w, h, DefaultDepth(dpy, screen));
	XSetForeground(dpy, drw->gc, scheme[SchemeNorm][ColBg].pixel);
	XFillRectangle(dpy, tippix, drw->gc, 0, 0, w, h);
	for (i = 0; i < n; i++)
		tipline(line[i], TIPPAD + i * lh, 1);
	drw_setscheme(drw, scheme[SchemeNorm]);

	XMoveResizeWindow(dpy, tipwin, x, tipy, w, h);
	XSetWindowBackgroundPixmap(dpy, tipwin, tippix);
	XClearWindow(dpy, tipwin);
	XMapRaised(dpy, tipwin);
	/* the bar's pixmap was borrowed for the lines */
	drawbar(m);
}

/* root _CHADWM_TIP changed: "SEQ\ntext" from barhover.sh */
static void
tipupdate(void)
{
	Atom type;
	int format;
	unsigned long n, extra;
	unsigned char *data = NULL;
	char *nl;

	if (XGetWindowProperty(dpy, root, tipatom, 0, 16384, False, AnyPropertyType,
	                       &type, &format, &n, &extra, &data) != Success || !data)
		return;
	if (format == 8 && tipid && (nl = strchr((char *)data, '\n'))) {
		*nl = '\0';
		if ((unsigned int)strtoul((char *)data, NULL, 10) == tipseq)
			tipshow(nl + 1);
	}
	XFree(data);
}

/* pointer moved on a bar: the status module under it, if any */
static void
tipmotion(Monitor *m, int x, int xroot)
{
	int i;

	if (x > m->ww - (int)TEXTW(stext))
		for (i = nblocks; i-- > 0;)
			if (x >= blockx[i]) {
				tiphover(m, blockid[i], xroot - x + blockx[i]);
				return;
			}
	tiphover(m, 0, 0);
}

void
leavenotify(XEvent *e)
{
	Monitor *m;

	for (m = mons; m; m = m->next)
		if (e->xcrossing.window == m->barwin) {
			tiphover(m, 0, 0);
			return;
		}
}
