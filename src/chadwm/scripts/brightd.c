/* brightd — adaptive Fn-key brightness ramping for chadwm (C port of
 * brightd.py, 2026-09-28; same algorithm and tunables, see brightd.py for the
 * why). Differences from the Python version:
 *   - opens only input devices that advertise KEY_BRIGHTNESS{UP,DOWN} and sets
 *     an EVIOCSMASK so the kernel delivers nothing but those two keys (the
 *     Python version read all 16 devices and woke on every pointer motion);
 *   - sleeps indefinitely while idle (was a 50 ms poll = 20 wakeups/s);
 *   - hotplug via inotify on /dev/input (was a 5 s rescan);
 *   - ~100 KB RSS instead of 12.5 MB.
 * Env: BRIGHT_DEV (acpi_video0), BRIGHT_STEP (1 %), BRIGHT_HOLD (0.2 s),
 *      BRIGHT_STATE (~/.local/state/brightness-pct). Each save also goes to
 *      /var/lib/backlight/level, which udev restores at boot.
 * Build: cc -O2 -s -o ~/.local/bin/brightd brightd.c
 */
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/input.h>
#include <math.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/inotify.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#define MAXDEV 16
#define GRACE 3.00           /* s without a press ends a hold sequence */
#define PULSE_GAP 0.02       /* s: longer gap = new pulse measurement */
#define STEPS_PER_PULSE 20.0 /* ramp budget per pulse = overshoot bound */
#define DEVICE_MIN_INTERVAL 0.0012 /* s between backlight writes */
#define CREDIT_RATE_CAP 700.0      /* steps/s ceiling for a continuous hold */

static struct { int fd; char name[32]; } dev[MAXDEV];
static int ndev;
static int bfd, maxb, cur, step_units;
static char bright_path[PATH_MAX], state_path[PATH_MAX];
static const char *devname;

static double now(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return ts.tv_sec + ts.tv_nsec / 1e9;
}

static int read_int(const char *path, int def)
{
	char b[32];
	int fd = open(path, O_RDONLY | O_CLOEXEC), n;
	if (fd < 0)
		return def;
	n = read(fd, b, sizeof b - 1);
	close(fd);
	if (n <= 0)
		return def;
	b[n] = 0;
	return atoi(b);
}

/* Also save to the state the boot restore reads (udev ->
 * /usr/local/sbin/backlight-state add): "<dev> <brightness> <max>". Skipped
 * while the lock screen forces 100 %, like backlight-state change. */
#define SHARED_DIR "/var/lib/backlight"
static void write_shared(void)
{
	char b[64];
	int fd, n;
	if (access(SHARED_DIR "/locked", F_OK) == 0)
		return;
	n = snprintf(b, sizeof b, "%s %d %d\n", devname, cur, maxb);
	if ((fd = open(SHARED_DIR "/.level.brightd", O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0664)) < 0)
		return;
	fchmod(fd, 0664);
	if (write(fd, b, n) == n && close(fd) == 0)
		rename(SHARED_DIR "/.level.brightd", SHARED_DIR "/level");
	else
		unlink(SHARED_DIR "/.level.brightd");
}

static void write_state(void)
{
	char tmp[PATH_MAX + 8], b[16], *s;
	int fd, n;
	if ((s = strrchr(state_path, '/'))) {
		*s = 0;
		mkdir(state_path, 0755);
		*s = '/';
	}
	snprintf(tmp, sizeof tmp, "%s.tmp", state_path);
	n = snprintf(b, sizeof b, "%ld\n", lround(cur * 100.0 / maxb));
	if ((fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0644)) < 0)
		return;
	if (write(fd, b, n) == n && close(fd) == 0)
		rename(tmp, state_path);
	else
		unlink(tmp);
	write_shared();
}

static int step(int dir)
{
	char b[16];
	int n, nv = cur + dir * step_units;
	nv = nv < 1 ? 1 : nv > maxb ? maxb : nv;
	if (nv == cur)
		return 0;
	n = snprintf(b, sizeof b, "%d", nv);
	if (pwrite(bfd, b, n, 0) == n) {
		cur = nv;
		return 1;
	}
	cur = read_int(bright_path, cur);
	return 0;
}

#define BIT(a, n) ((a)[(n) / 8] >> ((n) % 8) & 1)

/* open /dev/input/NAME if it has a brightness key; mask everything else */
static void try_open(const char *name)
{
	unsigned char keys[KEY_MAX / 8 + 1] = {0}, m[KEY_MAX / 8 + 1] = {0};
	struct input_mask im;
	char path[64];
	int fd, i, t;

	if (strncmp(name, "event", 5) || ndev >= MAXDEV)
		return;
	for (i = 0; i < ndev; i++)
		if (!strcmp(dev[i].name, name))
			return;
	snprintf(path, sizeof path, "/dev/input/%s", name);
	if ((fd = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC)) < 0)
		return;
	if (ioctl(fd, EVIOCGBIT(EV_KEY, sizeof keys), keys) < 0 ||
	    !(BIT(keys, KEY_BRIGHTNESSDOWN) || BIT(keys, KEY_BRIGHTNESSUP))) {
		close(fd);
		return;
	}
	/* kernel-side filter: only the two keys reach us (empty masks elsewhere) */
	for (t = 1; t < EV_CNT; t++) {
		if (t == EV_SYN)
			continue;
		memset(m, 0, sizeof m);
		if (t == EV_KEY) {
			m[KEY_BRIGHTNESSDOWN / 8] |= 1 << KEY_BRIGHTNESSDOWN % 8;
			m[KEY_BRIGHTNESSUP / 8] |= 1 << KEY_BRIGHTNESSUP % 8;
		}
		im.type = t;
		im.codes_size = sizeof m;
		im.codes_ptr = (unsigned long)m;
		ioctl(fd, EVIOCSMASK, &im); /* best effort: old kernels just wake more */
	}
	dev[ndev].fd = fd;
	snprintf(dev[ndev].name, sizeof dev[ndev].name, "%s", name);
	ndev++;
}

static void scan(void)
{
	DIR *d = opendir("/dev/input");
	struct dirent *e;
	if (!d)
		return;
	while ((e = readdir(d)))
		try_open(e->d_name);
	closedir(d);
}

static void drop(int i)
{
	close(dev[i].fd);
	dev[i] = dev[--ndev];
}

int main(void)
{
	const char *e;
	double step_pct = (e = getenv("BRIGHT_STEP")) ? atof(e) : 1.0;
	double hold = (e = getenv("BRIGHT_HOLD")) ? atof(e) : 0.2;
	double gaps[8], credit = 0, last_press = 0, ramp_at = 0, last_step = 0, last_t, t;
	int ngaps = 0, seq = 0, down = 0, last_dir = 0, ino, i;
	char path[PATH_MAX];
	struct pollfd pfd[MAXDEV + 1];

	if (!(devname = getenv("BRIGHT_DEV")))
		devname = "acpi_video0";
	snprintf(path, sizeof path, "/sys/class/backlight/%s/max_brightness", devname);
	snprintf(bright_path, sizeof bright_path, "/sys/class/backlight/%s/brightness", devname);
	if ((e = getenv("BRIGHT_STATE")))
		snprintf(state_path, sizeof state_path, "%s", e);
	else
		snprintf(state_path, sizeof state_path, "%s/.local/state/brightness-pct",
		         getenv("HOME") ? getenv("HOME") : "/tmp");
	if ((maxb = read_int(path, 0)) <= 0) {
		fprintf(stderr, "brightd: no usable backlight at %s\n", path);
		return 1;
	}
	step_units = lround(maxb * step_pct / 100.0);
	if (step_units < 1)
		step_units = 1;
	if ((bfd = open(bright_path, O_WRONLY | O_CLOEXEC)) < 0) {
		perror("brightd: open brightness");
		return 1;
	}
	ino = inotify_init1(IN_NONBLOCK | IN_CLOEXEC);
	if (ino >= 0)
		inotify_add_watch(ino, "/dev/input", IN_CREATE | IN_ATTRIB);
	scan();
	if (!ndev)
		fprintf(stderr, "brightd: no brightness-key device yet, waiting\n");
	cur = read_int(bright_path, maxb / 2);
	last_t = now();

	for (;;) {
		int n = 0, timeout = seq ? 2 : -1;
		for (i = 0; i < ndev; i++)
			pfd[n++] = (struct pollfd){ dev[i].fd, POLLIN, 0 };
		if (ino >= 0)
			pfd[n++] = (struct pollfd){ ino, POLLIN, 0 };
		if (poll(pfd, n, timeout) < 0 && errno != EINTR)
			return 1;
		t = now();

		if (ino >= 0 && pfd[n - 1].revents) {
			char buf[4096];
			while (read(ino, buf, sizeof buf) > 0)
				;
			scan();
		}
		for (i = ndev - 1; i >= 0; i--) {
			struct input_event ev[64];
			ssize_t r;
			int k;
			if (!pfd[i].revents)
				continue;
			r = read(dev[i].fd, ev, sizeof ev);
			if (r < 0 && errno == EAGAIN)
				continue;
			if (r <= 0 || (pfd[i].revents & (POLLERR | POLLHUP))) {
				drop(i);
				continue;
			}
			for (k = 0; k < r / (ssize_t)sizeof *ev; k++) {
				int dir;
				double et;
				if (ev[k].type != EV_KEY)
					continue;
				if (ev[k].code == KEY_BRIGHTNESSUP)
					dir = 1;
				else if (ev[k].code == KEY_BRIGHTNESSDOWN)
					dir = -1;
				else
					continue;
				if (ev[k].value == 0) { /* release */
					down = 0;
					continue;
				}
				et = now();
				if (last_press && et - last_press > 1.0) {
					/* long silence in an open sequence: resync, then step */
					cur = read_int(bright_path, cur);
					step(dir);
					last_press = et;
					last_dir = dir;
					ramp_at = et + hold; /* py kept the stale deadline: a tap here ramped */
					credit = 0;
					ngaps = 0;
					down = 1;
					continue;
				}
				if (!seq) { /* first press: a tap always moves one step */
					seq = 1;
					last_press = et;
					last_dir = dir;
					ramp_at = et + hold;
					credit = 0;
					ngaps = 0;
					cur = read_int(bright_path, cur);
					step(dir);
				} else {
					if (last_press && et - last_press > PULSE_GAP) {
						if (ngaps == 8)
							memmove(gaps, gaps + 1, 7 * sizeof *gaps), ngaps--;
						gaps[ngaps++] = et - last_press;
					}
					last_press = et;
					last_dir = dir;
				}
				down = 1;
			}
		}

		if (seq && !down && t - last_press > GRACE) { /* sequence over */
			seq = 0;
			ngaps = 0;
			credit = 0;
			last_press = 0;
			write_state();
		}

		double dt = t - last_t, mean_gap = 0, max_gap = 0, stale = 0;
		int held = down;
		last_t = t;
		if (!held && seq && ngaps && last_press) {
			int m = ngaps < 3 ? ngaps : 3;
			for (i = ngaps - m; i < ngaps; i++) {
				mean_gap += gaps[i];
				if (gaps[i] > max_gap)
					max_gap = gaps[i];
			}
			mean_gap /= m;
			stale = t - last_press;
			/* the EC's pulse gaps jitter: allow 1.2x the widest recent gap */
			held = stale < fmax(0.25, 1.2 * max_gap);
		}
		if (held && seq && t >= ramp_at) {
			double rate;
			if (down || !ngaps || mean_gap <= 0)
				rate = CREDIT_RATE_CAP;
			else {
				rate = fmin(CREDIT_RATE_CAP, STEPS_PER_PULSE / mean_gap);
				if (stale > max_gap) /* pulse overdue: taper, don't run on */
					rate *= 0.15;
			}
			credit = fmin(STEPS_PER_PULSE, credit + rate * dt);
			while (credit >= 1.0 && last_dir) {
				double s = now();
				if (s - last_step < DEVICE_MIN_INTERVAL) {
					struct timespec ts = { 0, (long)((DEVICE_MIN_INTERVAL - (s - last_step)) * 1e9) };
					nanosleep(&ts, NULL);
					continue;
				}
				last_step = now();
				if (step(last_dir))
					credit -= 1.0;
				else
					credit = 0;
			}
		}
	}
}
