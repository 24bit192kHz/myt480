/* hyde.c — Hyprland/HyDE window behaviour for chadwm (2026-09-28).
 *
 * - hydwindle: HyDE's default dwindle layout. Window i takes one side of
 *   the remaining area, split along the longer axis; every node keeps its
 *   own split direction (togglesplit, Super+J) and ratio (resizedir,
 *   Super+Shift+arrows) like Hyprland's dwindle tree.
 * - focusdir / movedir: Super+arrows / Super+Ctrl+Shift+arrows act on the
 *   window in that direction (floating: move by 30 px).
 * - dwindleresize: Hyprland's dwindle smart_resizing (DwindleAlgorithm::
 *   resizeTarget, 0.56.2): Super+Shift+arrows move the nearest divider in
 *   the arrow's direction; mouse resizes (Super+RMB, Super+X, dragging a
 *   gap / edge) move the dividers on the grabbed side and keep the window
 *   tiled.
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

/* lay out n clients dwindle-style inside x,y,w,h (outer gaps applied);
 * apply 0 only records the node geometry (nodew/nodeh/svert) */
static void
dwindlearea(Client **cs, int n, int x, int y, int w, int h, int ih, int iv, int apply)
{
	int i, vert, cw, ch;
	Client *c;

	for (i = 0; i < n; i++) {
		c = cs[i];
		c->nodew = w;
		c->nodeh = h;
		if (i == n - 1) {
			c->svert = -1;
			c->spref = -1;
			if (apply)
				resize(c, x, y, w - 2 * c->bw, h - 2 * c->bw, 0);
			break;
		}
		/* split the longer side, decided once when the node appears
		 * (HyDE dwindle:preserve_split); Super+J flips it */
		if (c->spref < 0)
			c->spref = w > h;
		vert = c->spref;
		if (c->splitflip)
			vert = !vert;
		c->svert = vert;
		if (vert) {
			cw = (w - iv) * c->sratio;
			if (apply)
				resize(c, x, y, cw - 2 * c->bw, h - 2 * c->bw, 0);
			x += cw + iv;
			w -= cw + iv;
		} else {
			ch = (h - ih) * c->sratio;
			if (apply)
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
dwindlegeom(Monitor *m, int special, int apply)
{
	Client *cs[256];
	int n, oh, ov, ih, iv;
	unsigned int nc;

	getgaps(m, &oh, &ov, &ih, &iv, &nc);
	if (!(n = splitclients(m, special, cs, LENGTH(cs))))
		return;
	if (smartgaps && n == 1)
		oh = ov = 0;
	dwindlearea(cs, n, m->wx + ov, m->wy + oh, m->ww - 2 * ov, m->wh - 2 * oh, ih, iv, apply);
}

static void
dwindlelayer(Monitor *m, int special)
{
	dwindlegeom(m, special, 1);
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
		int f = c->splitflip, p = c->spref;
		swapclients(c, t);
		/* split settings belong to the node, not the window */
		c->sratio = t->sratio; c->splitflip = t->splitflip; c->spref = t->spref;
		t->sratio = r; t->splitflip = f; t->spref = p;
		arrange(selmon);
		focus(c);
	}
}

/* Hyprland dwindle smart_resizing (DwindleAlgorithm::resizeTarget) on one
 * axis. chadwm's dwindle is a list: node k splits into cs[k] (first part,
 * left / top) and the rest, so the leaf cs[i] is the first part of node i
 * and lies in the second part of nodes i-1..0. corner: HC_* bits of the
 * grabbed corner / edge, 0 = keyboard (CORNER_NONE). The outer divider is
 * the nearest one on the grabbed side; it moves by d px. An inner divider
 * passed on the way (the window's other side) is re-solved so that edge
 * stays put. */
#define HC_LEFT   1
#define HC_RIGHT  2
#define HC_TOP    4
#define HC_BOTTOM 8

static float
nodelen(Client *k, int horiz, Monitor *m)
{
	int oh, ov, ih, iv;
	unsigned int nc;

	getgaps(m, &oh, &ov, &ih, &iv, &nc); /* the gaps dwindlearea split with */
	return horiz ? k->nodew - iv : k->nodeh - ih;
}

static int
splitaxis(Monitor *m, int special, Client **cs, int n, int i, int horiz, int d, int corner)
{
	int k, first, outer = -1, inner = -1, low = 0, high = 0, lo, hi;
	float len, orig = 0;

	if (!d)
		return 0;
	/* the window touches the area's low / high edge on this axis when no
	 * divider of that axis lies on that side of it */
	for (k = i; k >= 0; k--)
		if (k < n - 1 && cs[k]->svert == horiz) {
			if (k == i)
				high = 1;
			else
				low = 1;
		}
	if (!low && !high)
		return 0; /* spans the whole axis: nothing to move */
	lo = (corner & (horiz ? HC_LEFT : HC_TOP)) || !high;  /* Hyprland LEFT || DISPLAYRIGHT */
	hi = (corner & (horiz ? HC_RIGHT : HC_BOTTOM)) || !low; /* RIGHT || DISPLAYLEFT */
	for (k = i; k >= 0; k--) {
		if (k == n - 1 || cs[k]->svert != horiz)
			continue;
		first = k == i;
		if (outer < 0 && (!corner || (lo && !first) || (hi && first)))
			outer = k;
		else if (outer < 0 && inner < 0)
			inner = k;
	}
	if (outer < 0)
		return 0;
	if (inner >= 0) {
		len = nodelen(cs[inner], horiz, m);
		orig = inner == i ? len * cs[inner]->sratio : len * (1 - cs[inner]->sratio);
	}
	len = nodelen(cs[outer], horiz, m);
	if (len > 0)
		cs[outer]->sratio = MAX(0.05f, MIN(0.95f, cs[outer]->sratio + d / len));
	if (inner >= 0) {
		dwindlegeom(m, special, 0);
		len = nodelen(cs[inner], horiz, m);
		if (len > 0)
			cs[inner]->sratio = MAX(0.05f, MIN(0.95f, inner == i
			                    ? (orig - d) / len : 1 - (orig + d) / len));
	}
	return 1;
}

static void
dwindleresize(Client *c, int dx, int dy, int corner)
{
	Client *cs[256];
	int n, i, special = (c->tags & scratchtag) != 0, moved;

	n = splitclients(c->mon, special, cs, LENGTH(cs));
	for (i = 0; i < n && cs[i] != c; i++)
		;
	if (i == n || n < 2)
		return;
	moved = splitaxis(c->mon, special, cs, n, i, 1, dx, corner);
	if (moved && dy)
		dwindlegeom(c->mon, special, 0); /* fresh node sizes for the other axis */
	moved |= splitaxis(c->mon, special, cs, n, i, 0, dy, corner);
	if (moved)
		arrange(c->mon);
}

/* Hyprland resizeactive ±30 px (HyDE Super+Shift+arrows): tiled, the
 * nearest divider on that axis moves in the arrow's direction; floating,
 * the window grows / shrinks around its centre (DefaultFloatingAlgorithm) */
static void
resizedir(const Arg *arg)
{
	Client *c = selmon->sel;
	int dx = arg->i == 0 ? -HYDE_STEP : arg->i == 1 ? HYDE_STEP : 0;
	int dy = arg->i == 2 ? -HYDE_STEP : arg->i == 3 ? HYDE_STEP : 0;

	if (!c || c->isfullscreen)
		return;
	if (c->isfloating || !selmon->lt[selmon->sellt]->arrange) {
		resize(c, c->x - dx / 2, c->y - dy / 2, c->w + dx, c->h + dy, 1);
		return;
	}
	if (selmon->lt[selmon->sellt]->arrange != hydwindle) {
		/* master/stack layouts: nearest equivalent */
		if (dx)
			setmfact(&(Arg){ .f = dx > 0 ? +0.05 : -0.05 });
		else
			setcfact(&(Arg){ .f = dy > 0 ? +0.25 : -0.25 });
		return;
	}
	dwindleresize(c, dx, dy, 0);
}

static Cursor
edgecursor(int corner)
{
	int l = corner & HC_LEFT, r = corner & HC_RIGHT;

	return cursor[corner & HC_TOP ? (l ? CurEdgeTL : r ? CurEdgeTR : CurEdgeT)
	            : corner & HC_BOTTOM ? (l ? CurEdgeBL : r ? CurEdgeBR : CurEdgeB)
	            : l ? CurEdgeL : r ? CurEdgeR : CurResize]->cursor;
}

/* the window whose edge is within HYDE_BORDER_GRAB px of root point x,y
 * (the pointer is in a gap, outside every window): Hyprland
 * resize_on_border + extend_border_grab_area. corner gets the HC_* edges. */
#define HYDE_BORDER_GRAB 15
static Client *
edgeclient(int x, int y, int *corner)
{
	Client *c, *best = NULL;
	int bestd = HYDE_BORDER_GRAB + 1, d, e;

	for (c = selmon->stack; c; c = c->snext) {
		if (!ISVISIBLE(c) || HIDDEN(c) || c->isfullscreen)
			continue;
		if (x < c->x - HYDE_BORDER_GRAB || x >= c->x + WIDTH(c) + HYDE_BORDER_GRAB ||
		    y < c->y - HYDE_BORDER_GRAB || y >= c->y + HEIGHT(c) + HYDE_BORDER_GRAB)
			continue;
		e = 0;
		d = 0;
		if (x < c->x)                 { e |= HC_LEFT;   d = MAX(d, c->x - x); }
		else if (x >= c->x + WIDTH(c)) { e |= HC_RIGHT;  d = MAX(d, x - c->x - WIDTH(c) + 1); }
		if (y < c->y)                 { e |= HC_TOP;    d = MAX(d, c->y - y); }
		else if (y >= c->y + HEIGHT(c)) { e |= HC_BOTTOM; d = MAX(d, y - c->y - HEIGHT(c) + 1); }
		if (!e || d >= bestd)
			continue;
		/* tiled windows outside the dwindle layout have no divider to drag */
		if (!c->isfloating && selmon->lt[selmon->sellt]->arrange &&
		    selmon->lt[selmon->sellt]->arrange != hydwindle)
			continue;
		best = c;
		bestd = d;
		*corner = e;
	}
	return best;
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
