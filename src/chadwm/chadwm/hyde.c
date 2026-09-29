/* hyde.c — Hyprland/HyDE window behaviour for chadwm (2026-09-28).
 *
 * - hydwindle: HyDE's default dwindle layout. Window i takes one side of
 *   the remaining area, split along the longer axis; every node keeps its
 *   own split direction (togglesplit, Super+J) and ratio (resizedir,
 *   Super+Shift+arrows) like Hyprland's dwindle tree.
 * - focusdir / movedir: Super+arrows / Super+Ctrl+Shift+arrows act on the
 *   window in that direction (floating: move by 30 px).
 * - special workspace (Super+S): its tiled clients get their own dwindle
 *   over the monitor, above the current tag (dwm.c arrangemon/restack).
 * - droptoggle (Super+Alt+T): pyprland-style dropdown terminal, separate
 *   from the special workspace.
 */

#define HYDE_STEP 30 /* px, Hyprland resizeactive / moveactive step */

static int
issplittable(Client *c, int special)
{
	return c && !c->isfloating && !HIDDEN(c) && ISVISIBLE(c) &&
	       (special ? (c->tags & scratchtag) != 0 : !(c->tags & (scratchtag | droptag)));
}

/* lay out n clients dwindle-style inside x,y,w,h (outer gaps applied) */
static void
dwindlearea(Client **cs, int n, int x, int y, int w, int h, int ih, int iv)
{
	int i, vert, cw, ch;
	Client *c;

	for (i = 0; i < n; i++) {
		c = cs[i];
		c->nodew = w;
		c->nodeh = h;
		if (i == n - 1) {
			c->svert = -1;
			resize(c, x, y, w - 2 * c->bw, h - 2 * c->bw, 0);
			break;
		}
		vert = w > h; /* split the longer side, as Hyprland does */
		if (c->splitflip)
			vert = !vert;
		c->svert = vert;
		if (vert) {
			cw = (w - iv) * c->sratio;
			resize(c, x, y, cw - 2 * c->bw, h - 2 * c->bw, 0);
			x += cw + iv;
			w -= cw + iv;
		} else {
			ch = (h - ih) * c->sratio;
			resize(c, x, y, w - 2 * c->bw, ch - 2 * c->bw, 0);
			y += ch + ih;
			h -= ch + ih;
		}
	}
}

/* collect the split-tree members (normal layer or special overlay) */
static int
splitclients(Monitor *m, int special, Client **cs, int max)
{
	Client *c;
	int n = 0;

	for (c = m->clients; c && n < max; c = c->next)
		if (issplittable(c, special))
			cs[n++] = c;
	return n;
}

static void
dwindlelayer(Monitor *m, int special)
{
	Client *cs[256];
	int n, oh, ov, ih, iv;
	unsigned int nc;

	getgaps(m, &oh, &ov, &ih, &iv, &nc);
	if (!(n = splitclients(m, special, cs, LENGTH(cs))))
		return;
	if (smartgaps && n == 1)
		oh = ov = 0;
	dwindlearea(cs, n, m->wx + ov, m->wy + oh, m->ww - 2 * ov, m->wh - 2 * oh, ih, iv);
}

static void
hydwindle(Monitor *m)
{
	dwindlelayer(m, 0);
}

/* the node whose split separates c from its sibling */
static Client *
splitnode(Client *c, Client **cs, int n)
{
	int i;

	for (i = 0; i < n && cs[i] != c; i++)
		;
	if (i == n || n < 2)
		return NULL;
	return i == n - 1 ? cs[n - 2] : c;
}

static void
togglesplit(const Arg *arg)
{
	Client *cs[256], *c = selmon->sel, *node;
	int special, n;

	if (!c || c->isfloating || selmon->lt[selmon->sellt]->arrange != hydwindle)
		return;
	special = (c->tags & scratchtag) != 0;
	n = splitclients(selmon, special, cs, LENGTH(cs));
	if ((node = splitnode(c, cs, n))) {
		node->splitflip = !node->splitflip;
		arrange(selmon);
	}
}

/* d: 0 left, 1 right, 2 up, 3 down */
static Client *
dirclient(Client *s, int d, int tiledonly)
{
	Client *c, *best = NULL;
	int sx = s->x + WIDTH(s) / 2, sy = s->y + HEIGHT(s) / 2;
	long score, bestscore = 0;
	int special = (s->tags & scratchtag) != 0;

	for (c = s->mon->clients; c; c = c->next) {
		int cx, cy, dx, dy;
		if (c == s || !ISVISIBLE(c) || HIDDEN(c) || (c->tags & droptag))
			continue;
		if (tiledonly && c->isfloating)
			continue;
		/* stay inside the layer: special overlay or the normal tag */
		if (!!(c->tags & scratchtag) != special)
			continue;
		cx = c->x + WIDTH(c) / 2;
		cy = c->y + HEIGHT(c) / 2;
		dx = cx - sx;
		dy = cy - sy;
		switch (d) {
		case 0: if (c->x + WIDTH(c) > s->x + 1 || dx >= 0) continue; break;
		case 1: if (c->x < s->x + WIDTH(s) - 1 || dx <= 0) continue; break;
		case 2: if (c->y + HEIGHT(c) > s->y + 1 || dy >= 0) continue; break;
		case 3: if (c->y < s->y + HEIGHT(s) - 1 || dy <= 0) continue; break;
		}
		/* nearest along the axis, overlap on the other axis preferred */
		score = d < 2 ? (long)abs(dx) + 2L * abs(dy) : (long)abs(dy) + 2L * abs(dx);
		if (!best || score < bestscore) {
			best = c;
			bestscore = score;
		}
	}
	return best;
}

static void
focusdir(const Arg *arg)
{
	Client *c;

	if (!selmon->sel) {
		focus(NULL);
		return;
	}
	if (selmon->sel->isfullscreen && lockfullscreen)
		return;
	if ((c = dirclient(selmon->sel, arg->i, 0))) {
		focus(c);
		restack(selmon);
	} else if (mons->next) {
		focusmon(&(Arg){ .i = arg->i == 0 || arg->i == 2 ? -1 : +1 });
	}
}

static void
swapclients(Client *a, Client *b)
{
	Client *cs[512], *c, **pp;
	int n = 0, i;

	for (c = a->mon->clients; c && n < LENGTH(cs); c = c->next)
		cs[n++] = c == a ? b : c == b ? a : c;
	for (pp = &a->mon->clients, i = 0; i < n; i++) {
		*pp = cs[i];
		pp = &cs[i]->next;
	}
	*pp = NULL;
}

static void
movedir(const Arg *arg)
{
	Client *c = selmon->sel, *t;
	static const int dx[] = { -1, 1, 0, 0 }, dy[] = { 0, 0, -1, 1 };

	if (!c || c->isfullscreen)
		return;
	if (c->isfloating || !selmon->lt[selmon->sellt]->arrange) {
		resize(c, c->x + dx[arg->i] * HYDE_STEP, c->y + dy[arg->i] * HYDE_STEP,
		       c->w, c->h, 1);
		return;
	}
	if ((t = dirclient(c, arg->i, 1))) {
		float r = c->sratio;
		int f = c->splitflip;
		swapclients(c, t);
		/* split settings belong to the node, not the window */
		c->sratio = t->sratio; c->splitflip = t->splitflip;
		t->sratio = r; t->splitflip = f;
		arrange(selmon);
		focus(c);
	}
}

/* Hyprland resizeactive: grow/shrink the focused window by one step */
static void
resizedir(const Arg *arg)
{
	Client *c = selmon->sel, *cs[256];
	int horiz = arg->i < 2, grow = arg->i == 1 || arg->i == 3;
	int n, i, j, special, gap;

	if (!c || c->isfullscreen)
		return;
	if (c->isfloating || !selmon->lt[selmon->sellt]->arrange) {
		resize(c, c->x, c->y,
		       c->w + (horiz ? (grow ? HYDE_STEP : -HYDE_STEP) : 0),
		       c->h + (!horiz ? (grow ? HYDE_STEP : -HYDE_STEP) : 0), 1);
		return;
	}
	if (selmon->lt[selmon->sellt]->arrange != hydwindle) {
		/* master/stack layouts: nearest equivalent */
		if (horiz)
			setmfact(&(Arg){ .f = grow ? +0.05 : -0.05 });
		else
			setcfact(&(Arg){ .f = grow ? +0.25 : -0.25 });
		return;
	}
	special = (c->tags & scratchtag) != 0;
	n = splitclients(selmon, special, cs, LENGTH(cs));
	for (i = 0; i < n && cs[i] != c; i++)
		;
	if (i == n || n < 2)
		return;
	gap = horiz ? selmon->gappiv : selmon->gappih;
	/* own node: c is its first part, so growing c raises the ratio */
	if (i < n - 1 && cs[i]->svert == horiz) {
		int len = (horiz ? cs[i]->nodew : cs[i]->nodeh) - gap;
		if (len > 0)
			cs[i]->sratio += (grow ? 1.0f : -1.0f) * HYDE_STEP / len;
		j = i;
	} else {
		/* nearest ancestor on that axis: c is in its second part */
		for (j = i - 1; j >= 0 && cs[j]->svert != horiz; j--)
			;
		if (j < 0)
			return;
		{
			int len = (horiz ? cs[j]->nodew : cs[j]->nodeh) - gap;
			if (len > 0)
				cs[j]->sratio -= (grow ? 1.0f : -1.0f) * HYDE_STEP / len;
		}
	}
	cs[j]->sratio = MAX(0.1f, MIN(0.9f, cs[j]->sratio));
	arrange(selmon);
}

/* pyprland `toggle console`: a dropdown st, independent of Super+S
 * (dropshown / droptag live in dwm.c next to scratchshown) */

static void
droptoggle(const Arg *arg)
{
	Client *c;

	for (c = selmon->clients; c && !(c->tags & droptag); c = c->next)
		;
	if (!c) {
		dropshown = 1;
		spawn(&(Arg){ .v = (const char *[]){ "st", "-c", "dropterm", NULL } });
		return;
	}
	dropshown = !dropshown;
	arrange(selmon);
	if (dropshown) {
		XRaiseWindow(dpy, c->win);
		focus(c);
	} else {
		focus(NULL);
	}
}
