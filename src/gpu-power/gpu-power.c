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
 *   tune       reapply the current power source's clock offsets after a wake
 *   release    a program ended: back to what the policy or the hand said
 *   status
 *
 * Clock offsets: after the driver is loaded, the offsets of /etc/gpu-power.conf
 * (ac_gpc_offset, ac_mem_offset, batt_gpc_offset, batt_mem_offset, MHz, default 0)
 * are set through NVML for the current power source. Runtime suspend also clears
 * them: prime-run holds a GPU descriptor and tunes again before starting work.
 *
 * Runtime D3: with "rtd3 1" in /etc/gpu-power.conf every load uses NVIDIA's coarse-grained
 * runtime D3 (NVreg_DynamicPowerManagement=0x01, nvidia-drm modeset=0 fbdev=0, runtime PM
 * allowed): while the driver is loaded the GPU powers itself off (D3cold, rail off)
 * after the last program closes it and wakes on demand. On AC the GPU
 * then sleeps whenever it is idle instead of staying on; programs still start without a
 * module load. Changing the setting takes effect at the next load (gpu-power off; auto).
 *
 * Installed setuid root: the only thing a user can make it do is run modprobe
 * with the fixed arguments below and a fixed environment.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

/* Compile-time overrides let the hardware-independent tests use fixtures. */
#ifndef GPU
#define GPU      "/sys/bus/pci/devices/0000:01:00.0"
#endif
#ifndef AC
#define AC       "/sys/class/power_supply/AC/online"
#endif
#ifndef MODPROBE
#define MODPROBE "/usr/bin/modprobe"
#endif
#ifndef LOCK
#define LOCK     "/run/gpu-power.lock"
#endif
#ifndef USERS
#define USERS    "/run/gpu-power.users"
#endif
#ifndef MANUAL
#define MANUAL   "/run/gpu-power.manual"
#endif
#ifndef CONF
#define CONF     "/etc/gpu-power.conf"
#endif
#ifndef NVML
#define NVML     "/usr/lib/libnvidia-ml.so.1"
#endif
#ifndef NVIDIA_PARAMS
#define NVIDIA_PARAMS "/proc/driver/nvidia/params"
#endif
#ifndef NVIDIA_DEVICE
#define NVIDIA_DEVICE "/dev/nvidia0"
#endif
#ifndef NVIDIA_MODULE
#define NVIDIA_MODULE "/sys/module/nvidia"
#endif

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

/* With the root port disabled in the firmware, 01:00.0 is another device */
static int gpu_present(void)
{
	char v[16];
	return read_word(GPU "/vendor", v, sizeof(v)) == 0 && !strcmp(v, "0x10de");
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

/* "rtd3 1" in CONF: load the driver with coarse-grained runtime D3 */
static int conf_rtd3(void)
{
	char line[96], key[48];
	int v, r = 0;
	FILE *f = fopen(CONF, "r");

	if (!f)
		return 0;
	while (fgets(line, sizeof(line), f))
		if (sscanf(line, "%47s %d", key, &v) == 2 && !strcmp(key, "rtd3"))
			r = v == 1;
	fclose(f);
	return r;
}

static int status(void)
{
	char state[16] = "?";
	char runtime[32] = "?", line[256];
	unsigned active;
	FILE *params;
	const char *m = manual();

	if (!gpu_present() || read_word(GPU "/power_state", state, sizeof(state))) {
		puts("GPU is switched off in the firmware (dgpu on, then reboot)");
		return 2;
	}
	printf("driver %s, GPU %s\n", exists(NVIDIA_MODULE) ? "loaded" : "not loaded",
	       strcmp(state, "D3cold") ? "on" : "off");
	read_word(GPU "/power/runtime_status", runtime, sizeof(runtime));
	printf("runtime PM: %s, PCI %s\n", runtime, state);
	params = fopen(NVIDIA_PARAMS, "r");
	if (params) {
		while (fgets(line, sizeof(line), params))
			if (sscanf(line, "DynamicPowerManagement: %u", &active) == 1) {
				printf("runtime D3 parameter: 0x%02x (%s)\n", active,
				       active == 1 ? "coarse" : active == 2 ? "fine" :
				       active == 3 ? "driver default" : active == 0 ? "disabled" : "unknown");
				if (conf_rtd3() && active != 1)
					puts("runtime D3 config is pending: reload the driver to apply it");
				break;
			}
		fclose(params);
	}
	if (m)
		printf("mode: %s by hand (gpu-power auto returns to the AC/battery policy)\n", m);
	else
		printf("mode: automatic, %s -> GPU %s\n", on_ac() ? "on AC" : "on battery",
		       on_ac() ? "on" : "off unless a program uses it");
	return 0;
}

/* Offsets for the current power source from CONF; out-of-range values count as 0 */
static void conf_offsets(int *gpc, int *mem)
{
	char line[96], key[48];
	const char *src = on_ac() ? "ac_" : "batt_";
	int v;
	FILE *f = fopen(CONF, "r");

	*gpc = *mem = 0;
	if (!f)
		return;
	while (fgets(line, sizeof(line), f))
		if (sscanf(line, "%47s %d", key, &v) == 2 && !strncmp(key, src, strlen(src))) {
			const char *k = key + strlen(src);
			if (!strcmp(k, "gpc_offset") && v >= -200 && v <= 300)
				*gpc = v;
			else if (!strcmp(k, "mem_offset") && v >= -1000 && v <= 2000)
				*mem = v;
		}
	fclose(f);
}

/* PCI devices start with runtime power management forbidden ("on"). Without a
   driver the GPU then keeps its root port awake and the ACPI power resource never
   turns the rail off; "auto" lets the port and the GPU go to D3cold. The same
   setting lets the NVIDIA driver suspend the GPU at runtime (rtd3). */
static void allow_runtime_pm(void)
{
	FILE *f = fopen(GPU "/power/control", "w");

	if (f) {
		fputs("auto", f);
		fclose(f);
	}
}

/* Set the clock offsets through NVML (the driver must be loaded) */
static int tune(int quiet)
{
	int (*init)(void), (*shutdown)(void), (*handle)(unsigned, void **), (*set_gpc)(void *, int),
	    (*set_mem)(void *, int);
	void *lib, *dev;
	int gpc, mem;
	int rc = 1, gpc_rc, mem_rc;

	if (!exists(NVIDIA_DEVICE) || !exists(CONF))
		return 1;
	conf_offsets(&gpc, &mem);
	lib = dlopen(NVML, RTLD_NOW | RTLD_LOCAL);
	if (!lib)
		return 1;
	init = (int (*)(void))dlsym(lib, "nvmlInit_v2");
	shutdown = (int (*)(void))dlsym(lib, "nvmlShutdown");
	handle = (int (*)(unsigned, void **))dlsym(lib, "nvmlDeviceGetHandleByIndex_v2");
	set_gpc = (int (*)(void *, int))dlsym(lib, "nvmlDeviceSetGpcClkVfOffset");
	set_mem = (int (*)(void *, int))dlsym(lib, "nvmlDeviceSetMemClkVfOffset");
	if (init && shutdown && handle && set_gpc && set_mem && !init()) {
		if (!handle(0, &dev)) {
			gpc_rc = set_gpc(dev, gpc);
			mem_rc = set_mem(dev, mem);
			rc = gpc_rc || mem_rc;
			if (rc && !quiet)
				fprintf(stderr, "gpu-power: clock offsets %+d/%+d MHz not accepted (gpc=%d mem=%d)\n",
				        gpc, mem, gpc_rc, mem_rc);
		}
		shutdown();
	}
	dlclose(lib);
	return rc;
}

static int on(int quiet)
{
	static const char *const mods[] = { "nvidia", "nvidia_modeset", "nvidia_drm", "nvidia_uvm" };
	struct timespec ts = { 0, 50 * 1000 * 1000 };
	const int rtd3 = conf_rtd3();
	size_t i;

	if (!gpu_present()) {
		if (!quiet)
			fputs("gpu-power: GPU is switched off in the firmware (dgpu on, then reboot)\n", stderr);
		return 2;
	}
	for (i = 0; i < sizeof(mods) / sizeof(mods[0]); i++) {
		char *argv[] = { "modprobe", "-q", "--ignore-install", (char *)mods[i], NULL, NULL, NULL };
		if (rtd3 && i == 0)
			argv[4] = "NVreg_DynamicPowerManagement=0x01";
		if (rtd3 && i == 2) {	/* KMS and its fbdev client keep the GPU awake */
			argv[4] = "modeset=0";
			argv[5] = "fbdev=0";
		}
		if (run(argv)) {
			fprintf(stderr, "gpu-power: cannot load %s\n", mods[i]);
			return 1;
		}
	}
	/* udev creates the device nodes */
	for (i = 0; i < 60 && !exists(NVIDIA_DEVICE); i++)
		nanosleep(&ts, NULL);
	if (!exists(NVIDIA_DEVICE)) {
		fputs("gpu-power: NVIDIA device node did not appear\n", stderr);
		return 1;
	}
	tune(quiet);
	if (rtd3)
		allow_runtime_pm();
	return 0;
}

static int off(int quiet)
{
	char *argv[] = { "modprobe", "-r", "-q", "nvidia_uvm", "nvidia_drm", "nvidia_modeset",
			 "nvidia", NULL };

	allow_runtime_pm();
	if (!exists(NVIDIA_MODULE))
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
static int apply(int users_fd, int retune)
{
	int rc;

	if (!gpu_present())
		return 0;
	if (!strcmp(wanted(), "on")) {
		/* After work ends, NVML would wake an idle RTD3 GPU only to set
		   offsets which the next suspend clears. Defer to the next use. */
		if (!retune && conf_rtd3() && exists(NVIDIA_MODULE))
			return 0;
		return on(1);
	}
	if (flock(users_fd, LOCK_EX | LOCK_NB)) {
		if (errno == EWOULDBLOCK) {
			tune(1);	/* a program is using it: offsets of the new power source */
			return 0;
		}
		return 1;
	}
	rc = off(1);
	flock(users_fd, LOCK_UN);
	return rc;
}

/* A launcher takes USERS before loading the driver/opening its device. Checking
   modprobe alone leaves that interval unprotected. Never wait for USERS while
   holding the policy lock: the launcher needs that lock to finish starting. */
static int manual_off(int users_fd)
{
	int rc;

	if (flock(users_fd, LOCK_EX | LOCK_NB)) {
		if (errno == EWOULDBLOCK) {
			fputs("gpu-power: a prime-run program is using or starting the GPU, left on\n", stderr);
			return 3;
		}
		perror("gpu-power: users lock");
		return 1;
	}
	rc = off(0);
	flock(users_fd, LOCK_UN);
	return rc;
}

int main(int argc, char **argv)
{
	static const char *const cmds[] = { "on", "off", "auto", "apply", "use", "release", "status", "tune" };
	int fd, users, rc = 0;
	size_t c;

	for (c = 0; argc == 2 && c < sizeof(cmds) / sizeof(cmds[0]); c++)
		if (!strcmp(argv[1], cmds[c]))
			break;
	if (argc != 2 || c == sizeof(cmds) / sizeof(cmds[0])) {
		fputs("usage: gpu-power on|off|auto|apply|use|release|status|tune\n", stderr);
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
		rc = manual_off(users);
	} else if (!strcmp(argv[1], "auto")) {
		set_manual(NULL);
		rc = apply(users, 1);
	} else if (!strcmp(argv[1], "apply") || !strcmp(argv[1], "release")) {
		rc = apply(users, strcmp(argv[1], "release") != 0);
	} else if (!strcmp(argv[1], "use")) {
		rc = on(0);
	} else if (!strcmp(argv[1], "tune")) {
		rc = gpu_present() ? tune(0) : 2;
	}
	close(fd);
	return rc;
}
