/* Exercise real controller code with an emulated thinkpad_acpi command file.
 * No MSR, sysfs or fan hardware is accessed. */
#define _GNU_SOURCE
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static char commands[128][32];
static int command_count, fail_level, short_write;
static int test_open(const char *path, int flags, ...)
{
    if (!strcmp(path, "/proc/acpi/ibm/fan")) return 999;
    if (flags & O_CREAT) {
        va_list ap;
        va_start(ap, flags);
        mode_t mode = va_arg(ap, int);
        va_end(ap);
        return open(path, flags, mode);
    }
    return open(path, flags);
}
static ssize_t test_write(int fd, const void *buf, size_t size)
{
    if (fd != 999) return write(fd, buf, size);
    assert(command_count < 128 && size < sizeof commands[0]);
    memcpy(commands[command_count], buf, size);
    commands[command_count++][size] = 0;
    if (fail_level && !strncmp(buf, "level ", 6) && strncmp(buf, "level auto", 10)) {
        fail_level = 0;
        errno = EIO;
        return -1;
    }
    if (short_write) { short_write = 0; return size - 1; }
    return size;
}
static int test_close(int fd) { return fd == 999 ? 0 : close(fd); }
#define open test_open
#define write test_write
#define close test_close
#define main thermald_program_main
#include "../thermald.c"
#undef main
#undef open
#undef write
#undef close

static void reset(void)
{
    command_count = fail_level = short_write = 0;
    fan_idx = -1;
    fan_owned = dry_run = 0;
    last_fan_write = 0;
}
static int saw(const char *cmd)
{
    for (int i = 0; i < command_count; i++) if (!strcmp(commands[i], cmd)) return 1;
    return 0;
}
int main(void)
{
    struct conf c;
    conf_defaults(&c);
    for (int i = 0; i < 5; i++) assert(c.uv[i] == 0);
    set_key(&c, "fan_levels", "2:0:38,4:36:48,6:46:56,7:54:72,8:70:32767");
    assert(c.nfan == 5);
    set_key(&c, "fan_levels", "-1:0:40,9:0:40,2:50:40,2.5:0:40,bad");
    assert(c.nfan == 5 && c.fan[0].lvl == 2 && c.fan[4].lvl == 8);

    reset();
    fan_apply(&c, 35, 100);
    assert(fan_owned && fan_idx == 0 && saw("watchdog 120") && saw("level 2"));
    int before = command_count;
    fan_apply(&c, 35, 129);
    assert(command_count == before);
    fan_apply(&c, 35, 130);
    assert(command_count == before + 2);
    fan_apply(&c, 90, 131);
    assert(fan_idx == 4 && saw("level full-speed"));
    fan_apply(&c, 40, 132);
    assert(fan_idx == 1 && !strcmp(commands[command_count - 1], "level 4"));
    fan_release();
    assert(!fan_owned && fan_idx == -1 && saw("level auto"));
    before = command_count;
    fan_release();
    assert(command_count == before); /* already released; no redundant writes */

    reset();
    fail_level = 1;
    fan_apply(&c, 35, 200);
    assert(!fan_owned && fan_idx == -1 && saw("level auto"));
    fan_apply(&c, 90, 201);
    assert(fan_owned && fan_idx == 4); /* failed write cannot poison hysteresis */

    reset();
    short_write = 1;
    fan_apply(&c, 35, 300);
    assert(!fan_owned && fan_idx == -1 && saw("level auto"));

    reset();
    dry_run = 1;
    fan_apply(&c, 90, 400);
    fan_release();
    assert(command_count == 0); /* --test must never issue hardware writes */
    puts("PASS: safe defaults, invalid curves, hysteresis, rapid heating, watchdog heartbeat, release, I/O failures and dry-run");
    return 0;
}
