/* thermald.c — unified fan + undervolt + RAPL daemon, T480 i7-8650U.
 *
 * Replaces throttled (python, ~32MB RSS) + thinkfan with one static binary.
 * Config: /etc/thermald.conf (key value per line), see thermald.conf.
 *
 * v2 (2026-09-28): same behavior, cheaper to run and to ship.
 *  - integer fixed-point math (milli-units): no libm, no float printf
 *  - no stdio/glob/localtime: raw syscalls + tiny log formatter
 *    (s6 catch-all logger timestamps lines)
 *  - one nanosleep per poll (was a 1s wake loop): SIGTERM still interrupts
 *  - coretemp/AC sysfs fds cached, re-resolved only when a read fails
 *    (was: glob + up to 32 hwmon opens every poll)
 *  - 0x150/0x610/0x1A2 are package scope: written once via cpu0
 *    (was: every CPU, 8x mailbox commands + 8x dmesg lines)
 *  - poll period follows the active profile: ac_interval / batt_interval
 *    (poll_s is the fallback when a profile has no interval)
 *  - v2.1: MCHBAR power-limit mirror kept equal to MSR 0x610; fan level 8
 *    means \"full-speed\"
 *
 * Build: musl-gcc -Os -static -s -o thermald-t480 thermald.c  (see Makefile)
 * Run:   thermald [-c /etc/thermald.conf] [--once] [--test]
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#define MSR_OC_MAILBOX   0x150
#define MSR_TEMP_TARGET  0x1A2
#define MSR_RAPL_UNIT    0x606
#define MSR_PKG_PLIMIT   0x610
#define OC_CMD_UV_SET    0x8000001100000000ULL
#define OC_CMD_ICC_SET   0x8000001700000000ULL

enum { PL_CORE, PL_GPU, PL_CACHE, PL_UNCORE, PL_ANALOGIO };
static const char *const pn[5] = { "CORE", "GPU", "CACHE", "UNCORE", "ANALOGIO" };

#define MAXLVL 16
#define CONF_PATH "/etc/thermald.conf"
#define FAN_PATH "/proc/acpi/ibm/fan"

static volatile sig_atomic_t quit;
static int dry_run;
static void onterm(int s) { (void)s; quit = 1; }

/* ---------------- log: one write(2) per line ---------------- */
static char lb[256];
static size_t ll;
static void ls_(const char *s) { while (*s && ll < sizeof lb - 1) lb[ll++] = *s++; }
static void lu_(unsigned long long v, unsigned base)
{
    char t[24];
    int n = 0;
    do t[n++] = "0123456789abcdef"[v % base]; while ((v /= base));
    while (n && ll < sizeof lb - 1) lb[ll++] = t[--n];
}
static void li_(long long v) { if (v < 0) { ls_("-"); lu_(-(unsigned long long)v, 10); } else lu_(v, 10); }
static void lx_(unsigned long long v) { ls_("0x"); lu_(v, 16); }
static void lm_(long long m) /* milli-units, trailing zeros trimmed */
{
    long long a = m < 0 ? -m : m;
    char f[4];
    int n = 3;
    if (m < 0) ls_("-");
    lu_(a / 1000, 10);
    if (!(a % 1000)) return;
    f[0] = '0' + a / 100 % 10; f[1] = '0' + a / 10 % 10; f[2] = '0' + a % 10; f[3] = 0;
    while (n > 1 && f[n - 1] == '0') f[--n] = 0;
    ls_("."); ls_(f);
}
static void lend(void) { lb[ll++] = '\n'; if (write(2, lb, ll) < 0) {} ll = 0; }
static void logs(const char *a, const char *b) { ls_(a); if (b) ls_(b); lend(); }
static void loge(const char *a) { ls_(a); ls_(": errno "); li_(errno); lend(); }

/* ---------------- config (milli-units) ---------------- */
struct profile { long long pl1_w, pl1_s, pl2_w, pl2_s, trip, interval, icc[3]; };
struct conf {
    char temp_path[128], ac_glob[128], gpu_temp_path[128];
    long long poll_s;
    struct profile ac, batt;
    long long uv[5];
    struct { int lvl, lo, hi; } fan[MAXLVL];
    int nfan;
};
#define K 1000LL
static void conf_defaults(struct conf *c)
{
    memset(c, 0, sizeof *c);
    strcpy(c->temp_path, "auto");
    strcpy(c->gpu_temp_path, "auto");
    strcpy(c->ac_glob, "/sys/class/power_supply/AC*/online");
    c->poll_s = 3 * K;
    c->ac   = (struct profile){ 35*K, 28*K, 60*K, 2, 90*K, 3*K, { 64*K, 31*K, 6*K } };
    c->batt = (struct profile){ 29*K, 28*K, 44*K, 2, 85*K, 6*K, { 40*K, 24*K, 6*K } };
    c->uv[PL_CORE] = c->uv[PL_CACHE] = -100*K;
    c->uv[PL_GPU] = c->uv[PL_UNCORE] = -50*K;
    c->nfan = 4;
    c->fan[0].lvl = 2; c->fan[0].lo = 0;  c->fan[0].hi = 44;
    c->fan[1].lvl = 4; c->fan[1].lo = 42; c->fan[1].hi = 54;
    c->fan[2].lvl = 5; c->fan[2].lo = 52; c->fan[2].hi = 64;
    c->fan[3].lvl = 7; c->fan[3].lo = 62; c->fan[3].hi = 32767;
}
/* "-0.002" -> -2 ; "44" -> 44000 ; stops at first non-number char */
static long long milli(const char *s, const char **end)
{
    long long i = 0, f = 0, sc = 100;
    int neg = (*s == '-');
    if (*s == '-' || *s == '+') s++;
    while (*s >= '0' && *s <= '9') i = i * 10 + (*s++ - '0');
    if (*s == '.') for (s++; *s >= '0' && *s <= '9'; s++) { f += (*s - '0') * sc; sc /= 10; }
    if (end) *end = s;
    i = i * K + f;
    return neg ? -i : i;
}
static void copy(char *d, size_t n, const char *s) { size_t i = 0; for (; s[i] && i < n - 1; i++) d[i] = s[i]; d[i] = 0; }
static void set_key(struct conf *c, const char *k, const char *v)
{
    static const struct { const char *k; size_t off; } num[] = {
#define P(name, field) { name, offsetof(struct conf, field) }
        P("poll_s", poll_s),
        P("ac_pl1_w", ac.pl1_w), P("ac_pl1_s", ac.pl1_s), P("ac_pl2_w", ac.pl2_w),
        P("ac_pl2_s", ac.pl2_s), P("ac_trip", ac.trip), P("ac_interval", ac.interval),
        P("batt_pl1_w", batt.pl1_w), P("batt_pl1_s", batt.pl1_s), P("batt_pl2_w", batt.pl2_w),
        P("batt_pl2_s", batt.pl2_s), P("batt_trip", batt.trip), P("batt_interval", batt.interval),
        P("uv_core", uv[PL_CORE]), P("uv_gpu", uv[PL_GPU]), P("uv_cache", uv[PL_CACHE]),
        P("uv_uncore", uv[PL_UNCORE]), P("uv_analogio", uv[PL_ANALOGIO]),
        P("icc_core_ac", ac.icc[0]), P("icc_gpu_ac", ac.icc[1]), P("icc_cache_ac", ac.icc[2]),
        P("icc_core_batt", batt.icc[0]), P("icc_gpu_batt", batt.icc[1]), P("icc_cache_batt", batt.icc[2]),
#undef P
    };
    size_t i;
    for (i = 0; i < sizeof num / sizeof *num; i++)
        if (!strcmp(k, num[i].k)) { *(long long *)((char *)c + num[i].off) = milli(v, 0); return; }
    if (!strcmp(k, "temp_path")) copy(c->temp_path, sizeof c->temp_path, v);
    else if (!strcmp(k, "gpu_temp_path")) copy(c->gpu_temp_path, sizeof c->gpu_temp_path, v);
    else if (!strcmp(k, "ac_glob")) copy(c->ac_glob, sizeof c->ac_glob, v);
    else if (!strcmp(k, "fan_levels")) {
        /* "2:0:44,4:42:54,5:52:64,7:62:32767"; malformed triples skipped */
        const char *p = v;
        int n = 0;
        while (*p && n < MAXLVL) {
            const char *e;
            long long l = milli(p, &e), lo, hi;
            if (*e != ':') goto skip;
            lo = milli(e + 1, &e);
            if (*e != ':') goto skip;
            hi = milli(e + 1, &e);
            c->fan[n].lvl = l / K; c->fan[n].lo = lo / K; c->fan[n].hi = hi / K; n++;
        skip:
            while (*p && *p != ',') p++;
            if (*p == ',') p++;
        }
        c->nfan = n;
    }
}
static int load_conf(struct conf *c, const char *path)
{
    char b[4096], *p, *e;
    ssize_t n;
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0) return -1;
    n = read(fd, b, sizeof b - 1);
    close(fd);
    if (n < 0) return -1;
    b[n] = 0;
    for (p = b; *p; p = e) {
        char *k, *v, *nl = strchr(p, '\n');
        e = nl ? nl + 1 : p + strlen(p);
        if (nl) *nl = 0;
        k = p + strspn(p, " \t");
        if (!*k || *k == '#') continue;
        v = k + strcspn(k, " \t=");
        if (*v) *v++ = 0;
        v += strspn(v, " \t=");
        v[strcspn(v, " \t\r")] = 0;
        if (*k && *v) set_key(c, k, v);
    }
    return 0;
}

/* ---------------- cached sysfs readers ---------------- */
static int read_long(int fd, long *out)
{
    char b[32];
    ssize_t n = pread(fd, b, sizeof b - 1, 0);
    if (n <= 0) return -1;
    b[n] = 0;
    *out = milli(b, 0) / K;
    return 0;
}
static int temp_fd = -1, ac_fd = -1, gpu_fd = -1;
/* /sys/class/hwmon/hwmonN/<file> of the hwmon called <name>: the ids shift across
   boots and may appear after we start; resolved lazily, dropped on any read failure */
static int hwmon_open(const char *name, const char *file, int quiet)
{
    char p[64];
    int i;
    for (i = 0; i < 32; i++) {
        char nm[16] = { 0 };
        int fd;
        memcpy(p, "/sys/class/hwmon/hwmon", 22);
        ll = 0; lu_(i, 10); memcpy(p + 22, lb, ll); ll = 0;
        strcpy(p + 22 + (i > 9 ? 2 : 1), "/name");
        fd = open(p, O_RDONLY | O_CLOEXEC);
        if (fd < 0) continue;
        if (read(fd, nm, sizeof nm - 1) > 0 && !strncmp(nm, name, strlen(name))) {
            close(fd);
            strcpy(strrchr(p, '/'), file);
            fd = open(p, O_RDONLY | O_CLOEXEC);
            if (fd >= 0 && !quiet) logs("temp ", p);
            return fd;
        }
        close(fd);
    }
    return -1;
}
static int temp_open(const struct conf *c)
{
    if (temp_fd >= 0) return 0;
    if (strcmp(c->temp_path, "auto")) {
        temp_fd = open(c->temp_path, O_RDONLY | O_CLOEXEC);
        if (temp_fd >= 0) logs("temp ", c->temp_path);
        return temp_fd < 0 ? -1 : 0;
    }
    temp_fd = hwmon_open("coretemp", "/temp1_input", 0);
    return temp_fd < 0 ? -1 : 0;
}
/* The discrete GPU as the EC reads it: thinkpad hwmon temp2 (-128 C while the GPU
   is off, and the read fails while it is in D3cold). gpu_temp_path auto | off | <file>;
   the hotter of CPU and GPU drives the fan. */
static int gpu_seen;
static int gpu_open(const struct conf *c)
{
    if (gpu_fd >= 0) return 0;
    if (!strcmp(c->gpu_temp_path, "off")) return -1;
    if (strcmp(c->gpu_temp_path, "auto")) {
        gpu_fd = open(c->gpu_temp_path, O_RDONLY | O_CLOEXEC);
        return gpu_fd < 0 ? -1 : 0;
    }
    gpu_fd = hwmon_open("thinkpad", "/temp2_input", 1);
    return gpu_fd < 0 ? -1 : 0;
}
/* ac_glob "<dir>/<prefix>*<suffix>": first matching entry wins.
   getdents64 directly: opendir() would drag malloc into the binary. */
struct lde { uint64_t ino; int64_t off; unsigned short reclen; unsigned char type; char name[]; };
static void ac_open(const struct conf *c)
{
    char dir[128], path[192], buf[2048];
    const char *star = strchr(c->ac_glob, '*'), *slash;
    size_t dl, pl;
    long n;
    int d;
    if (ac_fd >= 0) return;
    if (!star) { ac_fd = open(c->ac_glob, O_RDONLY | O_CLOEXEC); return; }
    for (slash = star; slash > c->ac_glob && *slash != '/'; slash--) ;
    dl = slash - c->ac_glob;
    pl = star - slash - 1;
    if (dl >= sizeof dir) return;
    memcpy(dir, c->ac_glob, dl); dir[dl] = 0;
    if ((d = open(dir, O_RDONLY | O_DIRECTORY | O_CLOEXEC)) < 0) return;
    while (ac_fd < 0 && (n = syscall(SYS_getdents64, d, buf, sizeof buf)) > 0) {
        long o;
        for (o = 0; o < n && ac_fd < 0; o += ((struct lde *)(buf + o))->reclen) {
            const char *nm = ((struct lde *)(buf + o))->name;
            if (nm[0] == '.' || strncmp(nm, slash + 1, pl) ||
                dl + strlen(nm) + strlen(star + 1) + 2 >= sizeof path) continue;
            strcpy(path, dir); strcat(path, "/"); strcat(path, nm); strcat(path, star + 1);
            ac_fd = open(path, O_RDONLY | O_CLOEXEC);
        }
    }
    close(d);
}
static int on_ac(const struct conf *c)
{
    long v;
    ac_open(c);
    if (ac_fd < 0) return 1; /* unknown: assume AC */
    if (read_long(ac_fd, &v)) { close(ac_fd); ac_fd = -1; return 1; }
    return v != 0;
}

/* ---------------- MSR (package scope, via cpu0) ---------------- */
static int msrfd = -1;
static uint64_t mread(uint32_t reg)
{
    uint64_t v = 0;
    if (pread(msrfd, &v, 8, reg) != 8) { ls_("msr read "); lx_(reg); loge(""); }
    return v;
}
static void mwrite(uint32_t reg, uint64_t v)
{
    if (dry_run) { ls_("DRY msr["); lx_(reg); ls_("] <- "); lx_(v); lend(); return; }
    if (pwrite(msrfd, &v, 8, reg) != 8) { ls_("msr write "); lx_(reg); loge(""); }
}
/* MCHBAR copy of the package power limit (MCHBAR+0x59a0). The CPU obeys
   the LOWER of this and MSR 0x610; coreboot leaves PL2 = 18.75 W enabled
   here, which silently capped the package at ~18.7 W whatever 0x610 said.
   Base from host bridge config 0x48; needs iomem=relaxed. */
static volatile uint64_t *mch_pl;
static void mch_open(void)
{
    unsigned char cfg[0x50];
    uint64_t base;
    void *m;
    int fd = open("/sys/bus/pci/devices/0000:00:00.0/config", O_RDONLY | O_CLOEXEC);
    if (fd < 0) return;
    if (pread(fd, cfg, sizeof cfg, 0) != (ssize_t)sizeof cfg) { close(fd); return; }
    close(fd);
    memcpy(&base, cfg + 0x48, 8);
    base &= ~0x7fffULL;
    if (!base) return;
    if ((fd = open("/dev/mem", O_RDWR | O_SYNC | O_CLOEXEC)) < 0) { loge("mchbar: /dev/mem"); return; }
    m = mmap(0, 0x1000, PROT_READ | PROT_WRITE, MAP_SHARED, fd, base + 0x5000);
    close(fd);
    if (m == MAP_FAILED) { loge("mchbar: mmap"); return; }
    mch_pl = (volatile uint64_t *)((char *)m + 0x9a0);
}
static int mch_sync(uint64_t plreg)
{
    if (!mch_pl || *mch_pl == plreg) return 0;
    if ((*mch_pl >> 63) & 1) { logs("mchbar: PL register locked, cannot sync", 0); return 0; }
    if (dry_run) { ls_("DRY mchbar[0x59a0] <- "); lx_(plreg); lend(); return 1; }
    *mch_pl = plreg;
    return 1;
}

static long long rdiv(long long n, long long d) /* round half away from zero */
{
    return n >= 0 ? (n + d / 2) / d : -((-n + d / 2) / d);
}
/* smallest Y then Z with t <= 2^Y * (1 + Z/4) * tu, tu = 2^-tb s (t in ms) */
static int solve_tw(long long t_ms, int tb)
{
    int Y, Z;
    for (Y = 0; Y < 32; Y++)
        for (Z = 0; Z < 4; Z++)
            if ((unsigned long long)t_ms * 4 << tb <= (unsigned long long)(4 + Z) * 1000 << Y)
                return Y | (Z << 5);
    return 0;
}
static uint64_t pl_reg(const struct profile *p)
{
    uint64_t u = mread(MSR_RAPL_UNIT);
    int pb = u & 0xf, tb = (u >> 16) & 0xf;
    uint64_t pl1 = rdiv(p->pl1_w << pb, K), pl2 = rdiv(p->pl2_w << pb, K);
    return pl1 | 1ULL << 15 | 1ULL << 16 | (uint64_t)solve_tw(p->pl1_s, tb) << 17 |
           pl2 << 32 | 1ULL << 47 | (uint64_t)solve_tw(p->pl2_s, tb) << 49;
}
static uint64_t uv_encode(int plane, long long mv_m)
{
    long long v = rdiv(mv_m * 1024, 1000000); /* mV * 1.024 */
    return OC_CMD_UV_SET | (uint64_t)plane << 40 | (0xFFE00000ULL & ((uint64_t)(v & 0xFFF) << 21));
}
static uint64_t icc_encode(int plane, long long amp_m)
{
    return OC_CMD_ICC_SET | (uint64_t)plane << 40 | (uint64_t)rdiv(amp_m * 4, K);
}
/* TjMax - trip, or -1 when TjMax is implausible */
static long want_tcc(const struct profile *p, uint64_t tt)
{
    long tj = (tt >> 16) & 0xff;
    return (tj >= 40 && tj <= 125) ? ((tj - rdiv(p->trip, K)) & 0x3F) : -1;
}
/* 0x610/0x1A2 compare-skipped; 0x150 mailbox is write-only, always sent */
static void apply_power(const struct profile *p, const long long *uv, const char *name)
{
    uint64_t tt = mread(MSR_TEMP_TARGET), plreg = pl_reg(p);
    long off = want_tcc(p, tt);
    int i, wrote = 0;
    if (off < 0) logs("trip: bad TjMax, skip", 0);
    else if (((tt >> 24) & 0x3F) != (uint64_t)off) {
        mwrite(MSR_TEMP_TARGET, (tt & ~(0x3FULL << 24)) | (uint64_t)off << 24);
        wrote = 1;
    }
    if (mread(MSR_PKG_PLIMIT) != plreg) { mwrite(MSR_PKG_PLIMIT, plreg); wrote = 1; }
    if (mch_sync(plreg)) wrote = 1;
    ls_(name); ls_(": PL1 "); lm_(p->pl1_w); ls_("W/"); lm_(p->pl1_s); ls_("s PL2 ");
    lm_(p->pl2_w); ls_("W/"); lm_(p->pl2_s); ls_("s trip "); lm_(p->trip); ls_("C reg=");
    lx_(plreg); logs(wrote ? "" : " (in-sync, mailbox only)", 0);
    for (i = 0; i < 5; i++) {
        uint64_t v = uv_encode(i, uv[i]);
        mwrite(MSR_OC_MAILBOX, v);
        ls_(name); ls_(": UV "); ls_(pn[i]); ls_(" "); lm_(uv[i]); ls_("mV -> "); lx_(v); lend();
    }
    for (i = 0; i < 3; i++) {
        uint64_t v = icc_encode(i, p->icc[i]);
        mwrite(MSR_OC_MAILBOX, v);
        ls_(name); ls_(": ICCMAX "); ls_(pn[i]); ls_(" "); lm_(p->icc[i]); ls_("A -> "); lx_(v); lend();
    }
}
static int drifted(const struct profile *p)
{
    uint64_t tt = mread(MSR_TEMP_TARGET);
    long off = want_tcc(p, tt);
    return mread(MSR_PKG_PLIMIT) != pl_reg(p) || (mch_pl && *mch_pl != pl_reg(p)) || (off >= 0 && ((tt >> 24) & 0x3F) != (uint64_t)off);
}

/* ---------------- fan (thinkfan-style hysteresis) ---------------- */
static int fan_idx = -1;
static time_t last_fan_write;
static void fan_cmd(const char *cmd)
{
    int fd = open(FAN_PATH, O_WRONLY | O_CLOEXEC);
    if (fd < 0) { loge("fan open"); return; }
    if (write(fd, cmd, strlen(cmd)) < 0) { ls_("fan "); loge(cmd); }
    close(fd);
}
static void fan_write(const struct conf *c, int i, long tempc, time_t now)
{
    char cmd[24] = "level ";
    fan_idx = i;
    if (c->fan[i].lvl >= 8) strcpy(cmd + 6, "full-speed"); /* max regulated RPM */
    else { cmd[6] = '0' + c->fan[i].lvl; cmd[7] = 0; }
    ls_(dry_run ? "DRY fan <- " : "fan <- "); ls_(cmd); ls_(" ("); li_(tempc); ls_("C)"); lend();
    if (dry_run) return;
    fan_cmd(cmd);
    fan_cmd("watchdog 0"); /* a level write re-arms the EC watchdog */
    last_fan_write = now;
}
static void fan_apply(const struct conf *c, long tempc, time_t now)
{
    int i = fan_idx;
    if (!c->nfan) return;
    if (i < 0) {
        for (i = 0; i < c->nfan - 1 && tempc > c->fan[i].hi; i++) ;
        fan_write(c, i, tempc, now);
        return;
    }
    if (tempc > c->fan[i].hi && i < c->nfan - 1) i++;
    else if (tempc < c->fan[i].lo && i > 0) i--;
    if (i != fan_idx || now - last_fan_write > 60) fan_write(c, i, tempc, now);
}

int main(int argc, char **argv)
{
    static struct conf c, nc;
    const char *cpath = CONF_PATH;
    const long DRIFT_S = 300;
    struct sigaction sa;
    struct stat st;
    time_t conf_mt = 0, next_drift = 0;
    int once = 0, i, last_src = -1, need_power = 1, warned = 0;

    conf_defaults(&c);
    for (i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "-c") && i + 1 < argc) cpath = argv[++i];
        else if (!strcmp(argv[i], "--once")) once = 1;
        else if (!strcmp(argv[i], "--test")) dry_run = once = 1;
        else { logs("usage: thermald [-c conf] [--once] [--test]", 0); return 2; }
    }
    logs(load_conf(&c, cpath) ? "conf missing, defaults: " : "conf ", cpath);
    if (!stat(cpath, &st)) conf_mt = st.st_mtime;

    memset(&sa, 0, sizeof sa);
    sa.sa_handler = onterm; /* no SA_RESTART: nanosleep returns on signal */
    sigaction(SIGTERM, &sa, 0);
    sigaction(SIGINT, &sa, 0);

    if ((msrfd = open("/dev/cpu/0/msr", O_RDWR | O_CLOEXEC)) < 0) {
        loge("no /dev/cpu/0/msr (msr module?)");
        return 1;
    }
    mch_open();
    if (!dry_run) { fan_cmd("watchdog 0"); logs("fan watchdog disarmed", 0); }
    ls_("start (pid "); li_(getpid()); ls_(")"); lend();

    while (!quit) {
        long raw;
        int src = on_ac(&c) ? 0 : 1;
        const struct profile *p = src ? &c.batt : &c.ac;
        time_t now = time(0);
        long long per = p->interval > 0 ? p->interval : c.poll_s;
        struct timespec ts;

        if (!stat(cpath, &st) && st.st_mtime != conf_mt) {
            conf_mt = st.st_mtime;
            conf_defaults(&nc);
            if (!load_conf(&nc, cpath)) {
                c = nc;
                fan_idx = -1;
                need_power = 1;
                if (temp_fd >= 0) { close(temp_fd); temp_fd = -1; }
                if (gpu_fd >= 0) { close(gpu_fd); gpu_fd = -1; }
                if (ac_fd >= 0) { close(ac_fd); ac_fd = -1; }
                logs("conf reloaded", 0);
                continue;
            }
        }
        if (temp_open(&c)) {
            if (!warned++) logs("temp coretemp absent, holding fan", 0);
        } else if (read_long(temp_fd, &raw)) {
            logs("temp read fail, re-resolving", 0);
            close(temp_fd); temp_fd = -1;
        } else {
            long g;
            warned = 0;
            raw /= 1000;
            if (!gpu_open(&c)) {
                if (read_long(gpu_fd, &g) || g <= 0 || g >= 120000) {
                    close(gpu_fd); gpu_fd = -1;
                    if (gpu_seen) { logs("gpu sensor off", 0); gpu_seen = 0; }
                } else {
                    if (!gpu_seen) { logs("gpu sensor on (thinkpad temp2)", 0); gpu_seen = 1; }
                    if (g / 1000 > raw) raw = g / 1000;
                }
            }
            fan_apply(&c, raw, now);
        }
        if (src != last_src || need_power) {
            apply_power(p, c.uv, src ? "BATT" : "AC");
            last_src = src;
            need_power = 0;
            next_drift = now + DRIFT_S;
        } else if (now >= next_drift) {
            if (drifted(p)) { logs("drift detected, re-applying", 0); apply_power(p, c.uv, src ? "BATT" : "AC"); }
            next_drift = now + DRIFT_S;
        }
        if (once) break;
        if (per < 500) per = 500;
        ts.tv_sec = per / 1000;
        ts.tv_nsec = per % 1000 * 1000000;
        nanosleep(&ts, 0);
    }
    logs("exit", 0);
    return 0;
}
