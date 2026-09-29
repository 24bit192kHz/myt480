/* slock-ly v2: suckless slock fork, PAM password + fingerprint, clock UI.
 *
 * Auth
 *  - Two PAM services, as before: "slock-finger" (pam_fprintd only) and
 *    "slock-password" (pam_unix + faillock + keyring). Finger attempts can
 *    never touch the faillock tally.
 *  - Fingerprint runs in ONE long-lived worker thread for the whole lock.
 *    It is never orphaned or restarted by typing: a scan stays armed while
 *    you type, and a wrong password does not kill the sensor. pam_fprintd
 *    re-prompts internally on no-match (max-tries), and the worker loops
 *    when a transaction ends, with backoff only when the daemon is absent.
 *    v1 re-armed a max-tries=1/timeout=10 transaction on a timer and
 *    dropped it on the first keypress; the Claim/Cancel churn is what wedged
 *    the Validity sensor ("fingerprint sometimes broken").
 *  - Password attempts run in their own thread, one at a time.
 *  - Workers report through a pipe; the main loop sleeps in select() on the
 *    X fd + pipe, waking only for input, auth results, the minute tick, or
 *    an expiring status message (v1 woke every 100 ms).
 *  - On exit the process simply ends; open-fprintd watches the claimer's
 *    bus name and releases the sensor.
 *
 * Render
 *  - Translucent overlay: a 32-bit ARGB window with a black veil
 *    (veil_alpha), so the compositor shows the live desktop (the earth
 *    wallpaper) behind the clock. Without a compositor it is plain dark.
 *    Input is grabbed either way; nothing behind can be used.
 *  - Clock, date, masked dots, one status line, battery + power row.
 *  - Double-buffered per screen, fonts opened once.
 */
#define _GNU_SOURCE

#include <ctype.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <pwd.h>
#include <signal.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/select.h>
#include <sys/prctl.h>
#include <sys/wait.h>
#include <grp.h>
#include <time.h>
#include <unistd.h>
#include <linux/oom.h>
#include <security/pam_appl.h>
#include <sys/stat.h>
#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/keysym.h>
#include <X11/extensions/Xrandr.h>
#include <X11/extensions/Xrender.h>
#include <X11/Xft/Xft.h>

#include "config.h"

#define LEN(a) (sizeof(a) / sizeof(*(a)))

static const char *svc_pw = "slock-password";
static const char *svc_fp = "slock-finger";

/* worker -> main events (one byte each through evpipe) */
enum { EV_OK = 'k', EV_BADPW = 'p', EV_FPREADY = 'r', EV_FPBAD = 'f', EV_FPGONE = 'g' };

enum { ST_IDLE, ST_CHECK, ST_BADPW, ST_BADFP, ST_CONFIRM };

struct lock {
	int screen, w, h;
	Window root, win;
	Pixmap buf;
	Visual *vis;
	Colormap cmap;
	int depth;
	GC gc;
	Picture pic;               /* XRender view of buf */
	XftDraw *xd;
	XftFont *fclock, *fdate, *ftext, *fbar;
	XftColor fg, dim, err, ok, accent;
	int pw_x[3], pw_y, pw_w;   /* power row hit boxes */
};

static int evpipe[2];
static const char *user;

static void
die(const char *fmt, ...)
{
	va_list ap;
	va_start(ap, fmt);
	vfprintf(stderr, fmt, ap);
	va_end(ap);
	exit(1);
}

static void
post(char ev)
{
	while (write(evpipe[1], &ev, 1) < 0 && errno == EINTR)
		;
}

/* ---------------------------------------------------------------- PAM */

struct conv_ctx {
	const char *pw;   /* NULL for the fingerprint service */
};

static int
conv(int n, const struct pam_message **msg, struct pam_response **resp, void *data)
{
	struct conv_ctx *c = data;
	struct pam_response *r;
	int i;

	if (n <= 0 || !(r = calloc((size_t)n, sizeof(*r))))
		return PAM_CONV_ERR;
	for (i = 0; i < n; i++) {
		const char *m = msg[i]->msg ? msg[i]->msg : "";
		switch (msg[i]->msg_style) {
		case PAM_PROMPT_ECHO_OFF:
			/* only the password service may answer a prompt; an empty
			   answer from the finger service would poison pam_unix */
			if (!c->pw || !(r[i].resp = strdup(c->pw)))
				goto fail;
			break;
		case PAM_TEXT_INFO:
		case PAM_ERROR_MSG:
			if (!c->pw) {
				/* pam_fprintd: "Place your finger...", "Swipe...",
				   "Failed to match fingerprint" */
				if (strstr(m, "match") || strstr(m, "ailed"))
					post(EV_FPBAD);
				else if (strstr(m, "finger") || strstr(m, "Finger"))
					post(EV_FPREADY);
			}
			break;
		default:
			goto fail;
		}
	}
	*resp = r;
	return PAM_SUCCESS;
fail:
	for (i = 0; i < n; i++)
		if (r[i].resp) {
			explicit_bzero(r[i].resp, strlen(r[i].resp));
			free(r[i].resp);
		}
	free(r);
	return PAM_CONV_ERR;
}

static int
pam_try(const char *svc, const char *pw)
{
	struct conv_ctx c = { pw };
	struct pam_conv pc = { conv, &c };
	pam_handle_t *ph = NULL;
	int ret = pam_start(svc, user, &pc, &ph);

	if (ret == PAM_SUCCESS)
		ret = pam_authenticate(ph, 0);
	if (ret == PAM_SUCCESS)
		ret = pam_acct_mgmt(ph, 0);
	if (ph)
		pam_end(ph, ret);
	return ret == PAM_SUCCESS;
}

static void *
finger_worker(void *arg)
{
	int backoff = 2;
	(void)arg;

	for (;;) {
		time_t t0 = time(NULL);
		if (pam_try(svc_fp, NULL)) {
			post(EV_OK);
			return NULL;
		}
		/* a transaction that ends within 2s never reached the sensor:
		   daemon/device absent (e.g. right after resume). Back off. */
		if (time(NULL) - t0 < 2) {
			post(EV_FPGONE);
			sleep(backoff);
			if (backoff < 30)
				backoff *= 2;
		} else {
			backoff = 2;
		}
	}
}

static pthread_mutex_t pwlock = PTHREAD_MUTEX_INITIALIZER;
static char pwbuf[256];
static int pwbusy;

static void *
pw_worker(void *arg)
{
	int ok;
	(void)arg;

	ok = pam_try(svc_pw, pwbuf);
	pthread_mutex_lock(&pwlock);
	explicit_bzero(pwbuf, sizeof(pwbuf));
	pwbusy = 0;
	pthread_mutex_unlock(&pwlock);
	post(ok ? EV_OK : EV_BADPW);
	return NULL;
}

static int
start_pw(const char *pw, size_t len)
{
	pthread_t t;

	pthread_mutex_lock(&pwlock);
	if (pwbusy || len >= sizeof(pwbuf)) {
		pthread_mutex_unlock(&pwlock);
		return 0;
	}
	memcpy(pwbuf, pw, len);
	pwbuf[len] = '\0';
	pwbusy = 1;
	pthread_mutex_unlock(&pwlock);
	if (pthread_create(&t, NULL, pw_worker, NULL)) {
		pthread_mutex_lock(&pwlock);
		explicit_bzero(pwbuf, sizeof(pwbuf));
		pwbusy = 0;
		pthread_mutex_unlock(&pwlock);
		return 0;
	}
	pthread_detach(t);
	return 1;
}

/* ------------------------------------------------------------ battery */

static long
readlong(const char *dir, const char *name)
{
	char p[300], b[32];
	int fd;
	ssize_t n;

	snprintf(p, sizeof(p), "%s/%s", dir, name);
	if ((fd = open(p, O_RDONLY | O_CLOEXEC)) < 0)
		return -1;
	n = read(fd, b, sizeof(b) - 1);
	close(fd);
	if (n <= 0)
		return -1;
	b[n] = '\0';
	return strtol(b, NULL, 10);
}

/* combined percent over all batteries (weighted by full capacity) */
static int
battery_read(int *charging)
{
	const char *base = "/sys/class/power_supply";
	long now = 0, full = 0, capsum = 0, ncap = 0;
	struct dirent *de;
	DIR *d = opendir(base);

	*charging = 0;
	if (!d)
		return -1;
	while ((de = readdir(d))) {
		char dir[280];
		long en, ef, cap;
		if (de->d_name[0] == '.')
			continue;
		snprintf(dir, sizeof(dir), "%s/%s", base, de->d_name);
		if (!strncmp(de->d_name, "AC", 2)) {
			if (readlong(dir, "online") == 1)
				*charging = 1;
			continue;
		}
		if (strncmp(de->d_name, "BAT", 3))
			continue;
		en = readlong(dir, "energy_now");
		ef = readlong(dir, "energy_full");
		if (en < 0 || ef <= 0) {
			en = readlong(dir, "charge_now");
			ef = readlong(dir, "charge_full");
		}
		if (en >= 0 && ef > 0) {
			now += en;
			full += ef;
		} else if ((cap = readlong(dir, "capacity")) >= 0) {
			capsum += cap;
			ncap++;
		}
	}
	closedir(d);
	if (full > 0)
		return (int)((now * 100 + full / 2) / full);
	return ncap ? (int)(capsum / ncap) : -1;
}

/* every sysfs read is an ACPI/EC transaction: cache them between repaints */
static int
battery(int *charging)
{
	static time_t at;
	static int pct, chg;
	time_t now = time(NULL);

	if (!at || now - at >= 30 || now < at) {
		pct = battery_read(&chg);
		at = now;
	}
	*charging = chg;
	return pct;
}

/* ------------------------------------------------------------- render */

static int status = ST_IDLE, fp_ready, fp_gone, npw;
static time_t status_until, confirm_until;
static int confirm_idx = -1;

static XftFont *
openfont(Display *dpy, int scr, const char *name, int px)
{
	char spec[160];
	XftFont *f;

	snprintf(spec, sizeof(spec), "%s:pixelsize=%d", name, px < 6 ? 6 : px);
	if (!(f = XftFontOpenName(dpy, scr, spec)))
		f = XftFontOpenName(dpy, scr, "monospace");
	return f;
}

static int
textw(Display *dpy, XftFont *f, const char *s)
{
	XGlyphInfo g;
	XftTextExtentsUtf8(dpy, f, (const FcChar8 *)s, (int)strlen(s), &g);
	return g.xOff;
}

static void
text(Display *dpy, struct lock *l, XftFont *f, XftColor *c, int x, int y, const char *s)
{
	(void)dpy;
	XftDrawStringUtf8(l->xd, c, f, x, y, (const FcChar8 *)s, (int)strlen(s));
}

static void
ctext(Display *dpy, struct lock *l, XftFont *f, XftColor *c, int y, const char *s)
{
	text(dpy, l, f, c, (l->w - textw(dpy, f, s)) / 2, y, s);
}

static void
setup_render(Display *dpy, struct lock *l)
{
	int h = l->h;

	/* vis/cmap/depth were chosen in lockscreen() */
	l->buf = XCreatePixmap(dpy, l->win, l->w, l->h, l->depth);
	l->gc = XCreateGC(dpy, l->buf, 0, NULL);
	l->pic = XRenderCreatePicture(dpy, l->buf, XRenderFindVisualFormat(dpy, l->vis), 0, NULL);
	l->xd = XftDrawCreate(dpy, l->buf, l->vis, l->cmap);
	l->fclock = openfont(dpy, l->screen, font_clock, h * clock_pct / 100);
	l->fdate = openfont(dpy, l->screen, font_text, h * date_pct / 1000);
	l->ftext = openfont(dpy, l->screen, font_text, h * text_pct / 1000);
	l->fbar = openfont(dpy, l->screen, font_icon, h * bar_pct / 1000);
	XftColorAllocName(dpy, l->vis, l->cmap, col_fg, &l->fg);
	XftColorAllocName(dpy, l->vis, l->cmap, col_dim, &l->dim);
	XftColorAllocName(dpy, l->vis, l->cmap, col_err, &l->err);
	XftColorAllocName(dpy, l->vis, l->cmap, col_ok, &l->ok);
	XftColorAllocName(dpy, l->vis, l->cmap, col_accent, &l->accent);
}

static void
paint(Display *dpy, struct lock *l)
{
	/* veil: black at veil_alpha over whatever is behind (with a compositor
	   the desktop earth shows through; without one it is plain dark) */
	XRenderColor veil = { 0, 0, 0, veil_alpha };
	char clk[16], date[64], st[96], bat[32], dots[4 * 32 + 1];
	const char *icons[3] = { "\xef\x80\x91", "\xef\x80\xa1", "\xef\x86\x86" }; /* power, reload, moon */
	XftColor *sc = &l->dim;
	time_t now = time(NULL);
	struct tm tm;
	int cy = l->h * 42 / 100, i, n, pct, chg, gap, x;

	localtime_r(&now, &tm);
	strftime(clk, sizeof(clk), "%-I:%M %p", &tm);
	strftime(date, sizeof(date), "%A, %d %B", &tm);

	switch (status) {
	case ST_CHECK:   snprintf(st, sizeof(st), "checking\xe2\x80\xa6"); sc = &l->accent; break;
	case ST_BADPW:   snprintf(st, sizeof(st), "wrong password"); sc = &l->err; break;
	case ST_BADFP:   snprintf(st, sizeof(st), "fingerprint not recognised"); sc = &l->err; break;
	case ST_CONFIRM: snprintf(st, sizeof(st), "click again to %s",
	                          confirm_idx == 0 ? "power off" : confirm_idx == 1 ? "reboot" : "suspend");
	                 sc = &l->accent; break;
	default:
		snprintf(st, sizeof(st), "%s", fp_gone ? "type password (fingerprint unavailable)" :
		         fp_ready ? "touch the sensor or type password" : "type password");
	}

	XRenderFillRectangle(dpy, PictOpSrc, l->pic, &veil, 0, 0, l->w, l->h);
	ctext(dpy, l, l->fclock, &l->fg, cy, clk);
	ctext(dpy, l, l->fdate, &l->dim, cy + l->fdate->height * 2, date);

	n = npw > 32 ? 32 : npw;
	for (i = 0; i < n; i++)
		memcpy(dots + 4 * i, "\xe2\x97\x8f ", 4);   /* U+25CF + space */
	dots[n ? 4 * n - 1 : 0] = '\0';
	if (n)
		ctext(dpy, l, l->ftext, &l->fg, cy + l->fdate->height * 5, dots);
	ctext(dpy, l, l->ftext, sc, cy + l->fdate->height * 5 + l->ftext->height * 2, st);

	/* bottom bar: battery left, power row right */
	pct = battery(&chg);
	if (pct >= 0) {
		snprintf(bat, sizeof(bat), "%s %d%%", chg ? "\xef\x83\xa7" : "\xef\x89\x80", pct);
		text(dpy, l, l->fbar, &l->dim, l->h / 30, l->h - l->h / 30, bat);
	}
	gap = l->fbar->height * 2;
	l->pw_y = l->h - l->h / 30;
	l->pw_w = gap;
	x = l->w - l->h / 30 - 3 * gap;
	for (i = 0; i < 3; i++, x += gap) {
		l->pw_x[i] = x;
		text(dpy, l, l->fbar, confirm_idx == i ? &l->accent : &l->dim, x, l->pw_y, icons[i]);
	}
	XCopyArea(dpy, l->buf, l->win, l->gc, 0, 0, l->w, l->h, 0, 0);
}

static void
paintall(Display *dpy, struct lock **locks, int n)
{
	int i;
	for (i = 0; i < n; i++)
		paint(dpy, locks[i]);
	XFlush(dpy);
}

static void
runcmd(const char *cmd)
{
	pid_t p = fork();
	if (p == 0) {
		setsid();
		execl("/bin/sh", "sh", "-c", cmd, (char *)NULL);
		_exit(127);
	}
}

static void
click(Display *dpy, struct lock *l, int x, int y)
{
	int i;
	(void)dpy;

	if (y < l->pw_y - l->fbar->height * 2 || y > l->h)
		return;
	for (i = 0; i < 3; i++) {
		if (x < l->pw_x[i] - l->pw_w / 4 || x > l->pw_x[i] + l->pw_w * 3 / 4)
			continue;
		if (confirm_idx == i && time(NULL) <= confirm_until) {
			confirm_idx = -1;
			status = ST_IDLE;
			runcmd(i == 0 ? cmd_poweroff : i == 1 ? cmd_reboot : cmd_suspend);
		} else {
			confirm_idx = i;
			confirm_until = time(NULL) + confirm_secs;
			status = ST_CONFIRM;
			status_until = confirm_until;
		}
		return;
	}
}

/* --------------------------------------------------------------- lock */

static void
dontkillme(void)
{
	int fd = open("/proc/self/oom_score_adj", O_WRONLY | O_CLOEXEC);
	char v[16];
	int n = snprintf(v, sizeof(v), "%d", OOM_SCORE_ADJ_MIN);

	if (fd < 0)
		return;
	if (write(fd, v, n) != n)
		die("slock: unable to disable OOM killer (not setuid?)\n");
	close(fd);
}

static void
onchld(int sig)
{
	int e = errno;
	pid_t p;
	(void)sig;
	while ((p = waitpid(-1, NULL, WNOHANG)) > 0)
		;   /* power-row children */
	errno = e;
}

static struct lock *
lockscreen(Display *dpy, int screen, int rr)
{
	char curs[8] = { 0 };
	struct lock *l;
	XSetWindowAttributes wa;
	XColor black = { 0 };
	Cursor inv;
	Pixmap cp;
	int i, pg = -1, kg = -1;

	if (!(l = calloc(1, sizeof(*l))))
		return NULL;
	l->screen = screen;
	l->root = RootWindow(dpy, screen);
	l->w = DisplayWidth(dpy, screen);
	l->h = DisplayHeight(dpy, screen);

	/* 32-bit ARGB window: a compositor blends it over the desktop, so the
	   lock is a translucent overlay. Input is grabbed regardless. */
	{
		XVisualInfo vi;
		if (XMatchVisualInfo(dpy, screen, 32, TrueColor, &vi)) {
			l->vis = vi.visual;
			l->depth = 32;
			l->cmap = XCreateColormap(dpy, l->root, vi.visual, AllocNone);
		} else {
			l->vis = DefaultVisual(dpy, screen);
			l->depth = DefaultDepth(dpy, screen);
			l->cmap = DefaultColormap(dpy, screen);
		}
	}
	wa.override_redirect = 1;
	wa.background_pixel = 0;
	wa.border_pixel = 0;
	wa.colormap = l->cmap;
	l->win = XCreateWindow(dpy, l->root, 0, 0, l->w, l->h, 0, l->depth,
	                       InputOutput, l->vis,
	                       CWOverrideRedirect | CWBackPixel | CWBorderPixel | CWColormap, &wa);
	cp = XCreateBitmapFromData(dpy, l->win, curs, 8, 8);
	inv = XCreatePixmapCursor(dpy, cp, cp, &black, &black, 0, 0);
	XDefineCursor(dpy, l->win, inv);
	setup_render(dpy, l);

	/* grab pointer and keyboard for up to 1s, else fail the lock */
	for (i = 0; i < 10; i++) {
		if (pg != GrabSuccess)
			pg = XGrabPointer(dpy, l->root, False, ButtonPressMask,
			                  GrabModeAsync, GrabModeAsync, None, None, CurrentTime);
		if (kg != GrabSuccess)
			kg = XGrabKeyboard(dpy, l->root, True, GrabModeAsync, GrabModeAsync, CurrentTime);
		if (pg == GrabSuccess && kg == GrabSuccess) {
			XMapRaised(dpy, l->win);
			if (rr)
				XRRSelectInput(dpy, l->win, RRScreenChangeNotifyMask);
			XSelectInput(dpy, l->root, SubstructureNotifyMask);
			XSelectInput(dpy, l->win, ExposureMask);
			return l;
		}
		if ((pg != AlreadyGrabbed && pg != GrabSuccess) ||
		    (kg != AlreadyGrabbed && kg != GrabSuccess))
			break;
		usleep(100000);
	}
	fprintf(stderr, "slock: unable to grab %s on screen %d\n",
	        pg != GrabSuccess ? "pointer" : "keyboard", screen);
	return NULL;
}

static void
run(Display *dpy, struct lock **locks, int nscreens, int rrbase)
{
	char pw[256], buf[32], ev;
	unsigned int len = 0;
	int xfd = ConnectionNumber(dpy), i, num;
	KeySym ks;
	XEvent e;

	paintall(dpy, locks, nscreens);
	for (;;) {
		fd_set fds;
		struct timeval tv;
		time_t now = time(NULL);
		long wait = 60 - now % 60;   /* next minute for the clock */
		int dirty = 0;

		if (status_until && status_until > now && status_until - now < wait)
			wait = status_until - now;
		tv.tv_sec = wait;
		tv.tv_usec = 0;
		FD_ZERO(&fds);
		FD_SET(xfd, &fds);
		FD_SET(evpipe[0], &fds);
		if (!XPending(dpy) &&
		    select((xfd > evpipe[0] ? xfd : evpipe[0]) + 1, &fds, NULL, NULL, &tv) < 0 &&
		    errno != EINTR)
			die("slock: select: %s\n", strerror(errno));

		now = time(NULL);
		if (status_until && now >= status_until) {
			status_until = 0;
			confirm_idx = -1;
			if (status != ST_CHECK)
				status = ST_IDLE;
		}
		if (!XPending(dpy))
			dirty = 1;   /* clock, status timeout or worker event */

		while (read(evpipe[0], &ev, 1) == 1) {
			dirty = 1;
			switch (ev) {
			case EV_OK:
				explicit_bzero(pw, sizeof(pw));
				return;
			case EV_BADPW:
				status = ST_BADPW;
				status_until = time(NULL) + err_secs;
				XBell(dpy, 0);
				break;
			case EV_FPREADY:
				fp_ready = 1;
				fp_gone = 0;
				break;
			case EV_FPBAD:
				if (status != ST_CHECK) {
					status = ST_BADFP;
					status_until = time(NULL) + err_secs;
				}
				break;
			case EV_FPGONE:
				fp_ready = 0;
				fp_gone = 1;
				break;
			}
		}

		while (XPending(dpy)) {
			XNextEvent(dpy, &e);
			if (e.type == KeyPress) {
				dirty = 1;
				explicit_bzero(buf, sizeof(buf));
				num = XLookupString(&e.xkey, buf, sizeof(buf), &ks, NULL);
				if (ks == XK_KP_Enter)
					ks = XK_Return;
				switch (ks) {
				case XK_Return:
					if (len && start_pw(pw, len)) {
						status = ST_CHECK;
						status_until = 0;
					}
					explicit_bzero(pw, sizeof(pw));
					len = 0;
					break;
				case XK_Escape:
					explicit_bzero(pw, sizeof(pw));
					len = 0;
					break;
				case XK_BackSpace:
					if (len)
						pw[--len] = '\0';
					break;
				default:
					if (num == 1 && buf[0] == 0x15) {   /* ctrl-u */
						explicit_bzero(pw, sizeof(pw));
						len = 0;
					} else if (num && !iscntrl((unsigned char)buf[0]) &&
					           len + num < sizeof(pw)) {
						memcpy(pw + len, buf, num);
						len += num;
						if (status == ST_BADPW || status == ST_BADFP) {
							status = ST_IDLE;
							status_until = 0;
						}
					}
				}
				npw = (int)len;
			} else if (e.type == ButtonPress) {
				dirty = 1;
				for (i = 0; i < nscreens; i++)
					if (e.xbutton.root == locks[i]->root)
						click(dpy, locks[i], e.xbutton.x_root, e.xbutton.y_root);
			} else if (rrbase >= 0 && e.type == rrbase + RRScreenChangeNotify) {
				XRRScreenChangeNotifyEvent *r = (XRRScreenChangeNotifyEvent *)&e;
				dirty = 1;
				for (i = 0; i < nscreens; i++)
					if (locks[i]->win == r->window)
						XResizeWindow(dpy, locks[i]->win, r->width, r->height);
			} else if (e.type == Expose) {
				dirty = 1;
			} else if (e.type == MapNotify || e.type == ConfigureNotify ||
			           e.type == CirculateNotify || e.type == ReparentNotify) {
				/* another window may be above the lock now. These four
				   events have the window at the same offset. */
				Window w = e.xmap.window;
				int own = 0;

				for (i = 0; i < nscreens; i++)
					own |= locks[i]->win == w;
				for (i = 0; !own && i < nscreens; i++)
					XRaiseWindow(dpy, locks[i]->win);
			}
		}
		if (dirty)
			paintall(dpy, locks, nscreens);
	}
}

int
main(int argc, char **argv)
{
	struct lock **locks;
	struct passwd *pwd;
	const char *target = getenv("SLOCK_USER");
	Display *dpy;
	pthread_t fpt;
	int i, n, rrbase = -1, rrerr;

	if (argc > 1 && !strcmp(argv[1], "-v")) {
		puts("slock-ly 2.0");
		return 0;
	}
	if (argc > 1)
		die("usage: slock-ly [-v]\n");

	dontkillme();
	{
		/* reaps power-row children */
		struct sigaction sa = { 0 };
		sa.sa_handler = onchld;
		sa.sa_flags = SA_RESTART | SA_NOCLDSTOP;
		sigaction(SIGCHLD, &sa, NULL);
	}

	/* PAM user: the X session owner, never the launcher */
	if (!target || !*target)
		die("slock: SLOCK_USER is not set; refusing to guess the user\n");
	if (!(pwd = getpwnam(target)) || !(user = strdup(pwd->pw_name)))
		die("slock: unknown user %s\n", target);
	if (pwd->pw_uid == 0)
		die("slock: refusing to lock for root\n");
	/* setuid hygiene: no user-controlled fontconfig paths as root */
	setenv("HOME", pwd->pw_dir, 1);
	unsetenv("FONTCONFIG_FILE");
	unsetenv("FONTCONFIG_PATH");
	unsetenv("XDG_CONFIG_HOME");
	unsetenv("XDG_CACHE_HOME");
	unsetenv("XDG_DATA_HOME");

	if (pipe2(evpipe, O_CLOEXEC | O_NONBLOCK))
		die("slock: pipe: %s\n", strerror(errno));
	if (!(dpy = XOpenDisplay(NULL)))
		die("slock: cannot open display\n");
	if (!XRRQueryExtension(dpy, &rrbase, &rrerr))
		rrbase = -1;

	n = ScreenCount(dpy);
	if (!(locks = calloc(n, sizeof(*locks))))
		die("slock: out of memory\n");
	for (i = 0; i < n; i++)
		if (!(locks[i] = lockscreen(dpy, i, rrbase >= 0)))
			return 1;
	XSync(dpy, False);

	/* sensor goes live immediately and stays live for the whole lock */
	if (pthread_create(&fpt, NULL, finger_worker, NULL) == 0)
		pthread_detach(fpt);
	else
		fp_gone = 1;

	run(dpy, locks, n, rrbase);
	return 0;
}
