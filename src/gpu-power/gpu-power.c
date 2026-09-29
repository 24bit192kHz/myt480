/* gpu-power: load or unload the NVIDIA driver of the MX150.
 *
 * coreboot powers the GPU at boot (`dgpu on`) and gives its root port an ACPI
 * power resource: without a driver the kernel puts the GPU into D3cold, with
 * the power rail off. So loading the driver turns the GPU on and unloading it
 * turns it off. X must not hold the DRM device (Option "AutoAddGPU" "false").
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
#define MODPROBE "/usr/bin/modprobe"
#define LOCK     "/run/gpu-power.lock"

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

static int status(void)
{
	char state[16] = "?";
	FILE *f = fopen(GPU "/power_state", "r");

	if (!f) {
		puts("GPU is switched off in the firmware (dgpu on, then reboot)");
		return 2;
	}
	if (fgets(state, sizeof(state), f))
		state[strcspn(state, "\n")] = '\0';
	fclose(f);
	printf("driver %s, GPU %s\n", exists("/sys/module/nvidia") ? "loaded" : "not loaded",
	       strcmp(state, "D3cold") ? "on" : "off");
	return 0;
}

static int on(void)
{
	static const char *const mods[] = { "nvidia", "nvidia_modeset", "nvidia_drm", "nvidia_uvm" };
	struct timespec ts = { 0, 50 * 1000 * 1000 };
	size_t i;

	if (!exists(GPU)) {
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

static int off(void)
{
	char *argv[] = { "modprobe", "-r", "-q", "nvidia_uvm", "nvidia_drm", "nvidia_modeset",
			 "nvidia", NULL };

	if (!exists("/sys/module/nvidia"))
		return 0;
	if (run(argv)) {
		fputs("gpu-power: the GPU is in use, left on\n", stderr);
		return 3;
	}
	return 0;
}

int main(int argc, char **argv)
{
	int fd, rc;

	if (argc != 2 || (strcmp(argv[1], "on") && strcmp(argv[1], "off") &&
			  strcmp(argv[1], "status"))) {
		fputs("usage: gpu-power on|off|status\n", stderr);
		return 64;
	}
	if (!strcmp(argv[1], "status"))
		return status();

	if (setgid(0) || setuid(0)) {
		fputs("gpu-power: not installed setuid root\n", stderr);
		return 77;
	}
	umask(077);
	fd = open(LOCK, O_RDWR | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0600);
	if (fd < 0 || flock(fd, LOCK_EX)) {
		perror("gpu-power: " LOCK);
		return 1;
	}
	rc = !strcmp(argv[1], "on") ? on() : off();
	close(fd);
	return rc;
}
