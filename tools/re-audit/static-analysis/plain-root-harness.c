/* Offline actual-source control-flow fixture. Device/file/process/privileged
 * calls are mocked, including calls in paths this fixture does not exercise.
 * Assumes successful matching-policy TPM unseal; does not test cryptography.
 * Run only through run-plain-root-fixture.sh, which pins the source and linker
 * wrappers. Never compile this fixture without the required --wrap options.
 */
#ifndef T480_OFFLINE_FIXTURE_WRAPPERS
#error "Use run-plain-root-fixture.sh with the reviewed linker wrapper list"
#endif
#define main early_main
#include "../../../kernel/early-init/init.c"
#undef main
#include <assert.h>
#include <setjmp.h>

static jmp_buf done;
static int master_loaded, mounted_plain_root, executed_root_init, last_tpm_command;
static int console_duplications;
static struct { char name[128]; int read; } files[64];
static int nextfd = 10;

static void check_fd(int fd)
{
    assert(fd >= 10 && fd < nextfd);
}

static void unexpected(void)
{
    fputs("Unexpected alternative path in the offline fixture; stopping locally.\n", stderr);
    abort();
}

int __wrap_open(const char *path, int flags, ...)
{
    (void)flags;
    if (strstr(path, "kmk.next")) { errno = ENOENT; return -1; }
    assert(nextfd < 64);
    snprintf(files[nextfd].name, sizeof files[nextfd].name, "%s", path);
    return nextfd++;
}
int __wrap_access(const char *path, int mode) { (void)path; (void)mode; return 0; }
ssize_t __wrap_read(int fd, void *buf, size_t len)
{
    check_fd(fd);
    const char *name = files[fd].name;
    if (!strcmp(name, "/dev/tpm0")) {
        unsigned char *response = buf;
        assert(len >= 14);
        memset(buf, 0, len);
        if (last_tpm_command == 0x176) { response[10] = 3; return 14; }
        return 10;
    }
    if (files[fd].read++) return 0;
    const char *data = "";
    if (!strcmp(name, "/proc/cmdline")) data = "rw init=/usr/local/sbin/t480-init";
    else if (strstr(name, "kmk.blob")) data = "matching-sealed-blob-fixture";
    else if (!strcmp(name, P_ROOT) || !strcmp(name, P_SWAP)) {
        memset(buf, 0, len); /* Both partitions are plain, not LUKS2. */
        return len;
    }
    size_t count = strlen(data);
    if (count > len) count = len;
    memcpy(buf, data, count);
    return count;
}
ssize_t __wrap_write(int fd, const void *buf, size_t len)
{
    check_fd(fd);
    if (!strcmp(files[fd].name, "/dev/tpm0")) {
        const unsigned char *command = buf;
        assert(len >= 10);
        last_tpm_command = command[6] << 24 | command[7] << 16 | command[8] << 8 | command[9];
    }
    return len;
}
int __wrap_close(int fd) { check_fd(fd); return 0; }
long __wrap_syscall(long number, ...)
{
    va_list args;
    va_start(args, number);
    if (number == SYS_getrandom) {
        void *buf = va_arg(args, void *);
        size_t len = va_arg(args, size_t);
        memset(buf, 1, len);
        va_end(args);
        return len;
    }
    assert(number == SYS_add_key);
    const char *type = va_arg(args, const char *);
    const char *desc = va_arg(args, const char *);
    assert(!strcmp(type, "trusted") && !strcmp(desc, "t480-kmk"));
    master_loaded++;
    va_end(args);
    return 123; /* Assumed successful matched-policy unseal, not a real key. */
}
int __wrap_mount(const char *source, const char *target, const char *type,
                 unsigned long flags, const void *data)
{
    (void)type; (void)flags; (void)data;
    if (source && !strcmp(source, P_ROOT) && !strcmp(target, "/newroot")) {
        assert(master_loaded == 1);
        mounted_plain_root++;
    }
    return 0;
}
int __wrap_umount(const char *path) { (void)path; return 0; }
int __wrap_umount2(const char *path, int flags) { (void)path; (void)flags; return 0; }
int __wrap_unlink(const char *path) { (void)path; return 0; }
int __wrap_chdir(const char *path) { (void)path; return 0; }
int __wrap_chroot(const char *path) { (void)path; return 0; }
int __wrap_dup2(int fd, int newfd)
{
    check_fd(fd);
    assert(!strcmp(files[fd].name, "/dev/console"));
    assert(newfd >= 0 && newfd <= 2);
    console_duplications++;
    return newfd;
}
int __wrap_execl(const char *path, const char *arg, ...)
{
    (void)arg;
    assert(!strcmp(path, "/usr/local/sbin/t480-init"));
    assert(master_loaded == 1 && mounted_plain_root == 1);
    executed_root_init++;
    longjmp(done, 1);
}
int __wrap_clock_gettime(clockid_t clock, struct timespec *value)
{
    assert(clock == CLOCK_MONOTONIC);
    value->tv_sec = 0;
    value->tv_nsec = 0;
    return 0;
}

/* These alternative branches are compiled from the source but are not part of
 * the plain-root success case. Fail locally rather than perform real work. */
int __wrap_reboot(int how) { (void)how; unexpected(); return -1; }
int __wrap_ioctl(int fd, unsigned long request, ...) { (void)fd; (void)request; unexpected(); return -1; }
ssize_t __wrap_pread(int fd, void *buf, size_t count, off_t offset)
{ (void)fd; (void)buf; (void)count; (void)offset; unexpected(); return -1; }
int __wrap_tcgetattr(int fd, struct termios *value) { (void)fd; (void)value; unexpected(); return -1; }
int __wrap_tcsetattr(int fd, int action, const struct termios *value)
{ (void)fd; (void)action; (void)value; unexpected(); return -1; }
int __wrap_pipe(int fds[2]) { (void)fds; unexpected(); return -1; }
pid_t __wrap_fork(void) { unexpected(); return -1; }
pid_t __wrap_waitpid(pid_t pid, int *status, int options)
{ (void)pid; (void)status; (void)options; unexpected(); return -1; }
void __wrap_sync(void) { unexpected(); }
int __wrap_pause(void) { unexpected(); return -1; }
void __wrap__exit(int status) { (void)status; unexpected(); abort(); }
int __wrap_stat(const char *path, struct stat *value) { (void)path; (void)value; unexpected(); return -1; }
int __wrap_nanosleep(const struct timespec *request, struct timespec *remaining)
{ (void)request; (void)remaining; unexpected(); return -1; }

int main(void)
{
    if (!setjmp(done)) early_main();
    assert(master_loaded == 1 && mounted_plain_root == 1 && executed_root_init == 1);
    assert(console_duplications == 3);
    puts("Confirmed source path: matched-policy master load -> plain root mount -> root init execution.");
    puts("Device, file, mount, key, process, reboot and exec calls were mocked; no hardware/key access performed.");
    return 0;
}
