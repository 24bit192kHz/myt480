/* gpu-power: load or unload the NVIDIA driver of the MX150.
 *
 * coreboot powers the GPU at boot (`dgpu on`) and gives its root port an ACPI
 * power resource: without a driver the kernel puts the GPU into D3cold, with
 * the power rail off. So loading the driver turns the GPU on and unloading it
 * turns it off. X must not hold the DRM device (Option "AutoAddGPU" "false").
 *
 * Policy: on AC the GPU is kept on (driver loaded, ready at once), on battery
 * it is off unless a program uses it (prime-run). `on` and `off` by hand win
 * over the policy until `auto` or the next boot.
 *
 *   on | off   by hand: keep the GPU on / off
 *   auto       back to the AC/battery policy, applied now
 *   apply      apply the policy (power-supply udev rule, boot)
 *   use        turn on for a program (prime-run)
 *   release    a program ended: back to what the policy or the hand said
 *   status
 *
 * Installed setuid root: the only thing a user can make it do is run modprobe
 * with the fixed arguments below and a fixed environment.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define GPU      "/sys/bus/pci/devices/0000:01:00.0"
#define AC       "/sys/class/power_supply/AC/online"
#define MODPROBE "/usr/bin/modprobe"
#define LOCK     "/run/gpu-power.lock"	/* one gpu-power at a time */
#define USERS    "/run/gpu-power.users"	/* prime-run holds a shared lock while a program runs */
#define MANUAL   "/run/gpu-power.manual"	/* "on" or "off" set by hand */

static char *const envp[] = { "PATH=/usr/bin:/bin", NULL };

static int run(char *const argv[])
{
	int st;
	pid_t p = fork();

	if (p < 0)
		return -1;
	if (p == 0) {
		int fd = open("/dev/null", O_WRONLY);
		if (fd >= 0) {
			dup2(fd, 2);
			close(fd);
		}
		execve(MODPROBE, argv, envp);
		_exit(127);
	}
	while (waitpid(p, &st, 0) < 0)
		if (errno != EINTR)
			return -1;
	return WIFEXITED(st) ? WEXITSTATUS(st) : -1;
}

static int exists(const char *path)
{
	struct stat sb;
	return stat(path, &sb) == 0;
}

static int read_word(const char *path, char *buf, size_t len)
{
	FILE *f = fopen(path, "r");

	if (!f)
		return -1;
	if (!fgets(buf, len, f))
		buf[0] = '\0';
	fclose(f);
	buf[strcspn(buf, "\n")] = '\0';
	return 0;
}

static int on_ac(void)
{
	char s[8];
	return read_word(AC, s, sizeof(s)) == 0 && s[0] == '1';
}

/* "on", "off" or NULL when nothing was set by hand */
static const char *manual(void)
{
	static char s[8];
	if (read_word(MANUAL, s, sizeof(s)))
		return NULL;
	return !strcmp(s, "on") ? "on" : !strcmp(s, "off") ? "off" : NULL;
}

static void set_manual(const char *what)
{
	if (!what) {
		unlink(MANUAL);
		return;
	}
	FILE *f = fopen(MANUAL, "w");
	if (f) {
		fputs(what, f);
		fclose(f);
	}
}

static const char *wanted(void)
{
	const char *m = manual();
	return m ? m : on_ac() ? "on" : "off";
}

static int status(void)
{
	char state[16] = "?";
	const char *m = manual();

	if (read_word(GPU "/power_state", state, sizeof(state))) {
		puts("GPU is switched off in the firmware (dgpu on, then reboot)");
		return 2;
	}
	printf("driver %s, GPU %s\n", exists("/sys/module/nvidia") ? "loaded" : "not loaded",
	       strcmp(state, "D3cold") ? "on" : "off");
	if (m)
		printf("mode: %s by hand (gpu-power auto returns to the AC/battery policy)\n", m);
	else
		printf("mode: automatic, %s -> GPU %s\n", on_ac() ? "on AC" : "on battery",
		       on_ac() ? "on" : "off unless a program uses it");
	return 0;
}

static int on(int quiet)
{
	static const char *const mods[] = { "nvidia", "nvidia_modeset", "nvidia_drm", "nvidia_uvm" };
	struct timespec ts = { 0, 50 * 1000 * 1000 };
	size_t i;

	if (!exists(GPU)) {
		if (!quiet)
			fputs("gpu-power: GPU is switched off in the firmware (dgpu on, then reboot)\n", stderr);
		return 2;
	}
	for (i = 0; i < sizeof(mods) / sizeof(mods[0]); i++) {
		char *argv[] = { "modprobe", "-q", "--ignore-install", (char *)mods[i], NULL };
		if (run(argv)) {
			fprintf(stderr, "gpu-power: cannot load %s\n", mods[i]);
			return 1;
		}
	}
	/* udev creates the device nodes */
	for (i = 0; i < 60 && !exists("/dev/nvidia0"); i++)
		nanosleep(&ts, NULL);
	return 0;
}

static int off(int quiet)
{
	char *argv[] = { "modprobe", "-r", "-q", "nvidia_uvm", "nvidia_drm", "nvidia_modeset",
			 "nvidia", NULL };

	if (!exists("/sys/module/nvidia"))
		return 0;
	if (run(argv)) {
		if (!quiet)
			fputs("gpu-power: the GPU is in use, left on\n", stderr);
		return 3;
	}
	return 0;
}

/* Bring the GPU to what the policy or the hand wants; never turn it off
   under a running program (prime-run holds USERS shared). */
static int apply(int users_fd)
{
	if (!exists(GPU))
		return 0;
	if (!strcmp(wanted(), "on"))
		return on(1);
	if (flock(users_fd, LOCK_EX | LOCK_NB)) {
		if (errno == EWOULDBLOCK)
			return 0;	/* a program is using it */
		return 1;
	}
	off(1);
	flock(users_fd, LOCK_UN);
	return 0;
}

int main(int argc, char **argv)
{
	static const char *const cmds[] = { "on", "off", "auto", "apply", "use", "release", "status" };
	int fd, users, rc = 0;
	size_t c;

	for (c = 0; argc == 2 && c < sizeof(cmds) / sizeof(cmds[0]); c++)
		if (!strcmp(argv[1], cmds[c]))
			break;
	if (argc != 2 || c == sizeof(cmds) / sizeof(cmds[0])) {
		fputs("usage: gpu-power on|off|auto|apply|use|release|status\n", stderr);
		return 64;
	}
	if (setgid(0) || setuid(0)) {
		fputs("gpu-power: not installed setuid root\n", stderr);
		return 77;
	}
	umask(022);
	users = open(USERS, O_RDONLY | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0644);
	if (users < 0) {
		perror("gpu-power: " USERS);
		return 1;
	}
	if (!strcmp(argv[1], "status"))
		return status();

	fd = open(LOCK, O_RDWR | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0600);
	if (fd < 0 || flock(fd, LOCK_EX)) {
		perror("gpu-power: " LOCK);
		return 1;
	}
	if (!strcmp(argv[1], "on")) {
		set_manual("on");
		rc = on(0);
	} else if (!strcmp(argv[1], "off")) {
		set_manual("off");
		rc = off(0);
	} else if (!strcmp(argv[1], "auto")) {
		set_manual(NULL);
		rc = apply(users);
	} else if (!strcmp(argv[1], "apply") || !strcmp(argv[1], "release")) {
		rc = apply(users);
	} else if (!strcmp(argv[1], "use")) {
		rc = on(0);
	}
	close(fd);
	return rc;
}
