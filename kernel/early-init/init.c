/*
 * T480 early init, built into linux-t480 (CONFIG_INITRAMFS_SOURCE). Static, musl.
 *
 * Opens the encrypted root and swap without a prompt: the volume keys are kernel
 * "encrypted" keys wrapped by a "trusted" key that the TPM only releases while
 * PCR 2 (coreboot, GRUB, grub.cfg) has the sealed value and PCR 8 is still zero.
 * PCR 8 is extended right after the attempt, so nothing that runs later (and no
 * other system started by this firmware) can get the key again before a reset.
 *
 * Each partition is looked at on its own: a LUKS2 header means dm-crypt, anything
 * else is used as before. The same kernel therefore boots the machine before and
 * after the conversion. On any failure the LUKS passphrase is asked on the
 * console; this program never starts a shell.
 *
 *   key directory:  <boot fs>/t480/   (third partition if it exists, else root:/boot/t480)
 *     kmk.blob      trusted key blob (keyctl pipe), policy PCR 2+8
 *     kmk.next      optional one-shot blob without policy (planned firmware flash)
 *     root.key      encrypted-key blob for the root volume key, swap.key likewise
 *     root.dm       "<data offset in 512-byte sectors> <sector size>", swap.dm likewise
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>
#include <linux/dm-ioctl.h>
#include <linux/fs.h>
#include <linux/reboot.h>
#include <sys/ioctl.h>
#include <sys/mount.h>
#include <sys/reboot.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>

#ifndef DISK
#define DISK "/dev/nvme0n1p"
#endif
#define P_ROOT DISK "1"
#define P_SWAP DISK "2"
#define P_BOOT DISK "3"
#define TPM_PARENT "0x81000101"
#define KEY_SPEC_USER_KEYRING (-4)
#define CRYPTSETUP "/bin/cryptsetup"

static int kmsg = -1, logfd = -1;	/* the T480 boots with printk.devkmsg=off: keep a copy in /dev */

static void say(const char *fmt, ...)
{
	char b[512];
	va_list ap;
	int n = snprintf(b, sizeof b, "<5>t480-early: ");

	va_start(ap, fmt);
	n += vsnprintf(b + n, sizeof b - n - 1, fmt, ap);
	va_end(ap);
	if (n > (int)sizeof b - 2)
		n = sizeof b - 2;
	b[n++] = '\n';
	if (kmsg >= 0)
		write(kmsg, b, n);
	if (logfd >= 0)
		write(logfd, b + 3, n - 3);
}

/* text for a person in front of the machine */
static void tell(const char *s)
{
	int fd = open("/dev/console", O_WRONLY | O_NOCTTY);

	if (fd >= 0) {
		write(fd, s, strlen(s));
		close(fd);
	}
}

static double now(void)
{
	struct timespec t;

	clock_gettime(CLOCK_MONOTONIC, &t);
	return t.tv_sec + t.tv_nsec / 1e9;
}

static void nap_ms(int ms)
{
	struct timespec t = { ms / 1000, (ms % 1000) * 1000000L };

	nanosleep(&t, NULL);
}

static int wait_for(const char *path, int ms)
{
	for (int i = 0; i < ms; i++) {
		if (!access(path, F_OK))
			return 0;
		nap_ms(1);
	}
	return -1;
}

static int slurp(const char *path, char *buf, size_t max)
{
	int fd = open(path, O_RDONLY), n, got = 0;

	if (fd < 0)
		return -1;
	while (got < (int)max - 1 && (n = read(fd, buf + got, max - 1 - got)) > 0)
		got += n;
	close(fd);
	while (got > 0 && (buf[got - 1] == '\n' || buf[got - 1] == ' '))
		got--;
	buf[got] = 0;
	return got;
}

static int spit(const char *path, const char *s)
{
	int fd = open(path, O_WRONLY), r;

	if (fd < 0)
		return -1;
	r = write(fd, s, strlen(s));
	close(fd);
	return r < 0 ? -1 : 0;
}

static int is_luks2(const char *dev)
{
	unsigned char h[8] = { 0 };
	int fd = open(dev, O_RDONLY);

	if (fd < 0)
		return 0;
	read(fd, h, 8);
	close(fd);
	return !memcmp(h, "LUKS\xba\xbe\x00\x02", 8);
}

/* ---- TPM 2.0, raw commands on /dev/tpm0 ---- */

static void be16(unsigned char **p, unsigned v) { *(*p)++ = v >> 8; *(*p)++ = v; }
static void be32(unsigned char **p, unsigned v) { be16(p, v >> 16); be16(p, v & 0xffff); }

/* returns the TPM response code, or -1; rsp gets the whole response */
static int tpm_cmd(int fd, unsigned char *cmd, unsigned char *end, unsigned char *rsp, int rmax)
{
	unsigned char *p = cmd + 2;
	int len = end - cmd, n;

	be32(&p, len);
	if (write(fd, cmd, len) != len)
		return -1;
	n = read(fd, rsp, rmax);
	if (n < 10)
		return -1;
	return rsp[6] << 24 | rsp[7] << 16 | rsp[8] << 8 | rsp[9];
}

/* policy session satisfied with the current PCR 2 and 8 (sha256); 0 on failure */
static unsigned tpm_policy_session(int fd)
{
	unsigned char c[128], r[128], *p = c;
	unsigned h;

	be16(&p, 0x8001); be32(&p, 0); be32(&p, 0x176);	/* StartAuthSession */
	be32(&p, 0x40000007); be32(&p, 0x40000007);	/* no salt key, not bound */
	be16(&p, 32);
	/* the nonce needs no secrecy and must not wait for the random pool */
	if (syscall(SYS_getrandom, p, 32, 0x0004 /* GRND_INSECURE */) != 32)
		memset(p, 0x5a, 32);
	p += 32;
	be16(&p, 0);					/* encryptedSalt */
	*p++ = 0x01;					/* TPM_SE_POLICY */
	be16(&p, 0x0010);				/* symmetric: none */
	be16(&p, 0x000b);				/* sha256 */
	if (tpm_cmd(fd, c, p, r, sizeof r))
		return 0;
	h = r[10] << 24 | r[11] << 16 | r[12] << 8 | r[13];

	p = c;
	be16(&p, 0x8001); be32(&p, 0); be32(&p, 0x17f);	/* PolicyPCR */
	be32(&p, h);
	be16(&p, 0);					/* use the current values */
	be32(&p, 1); be16(&p, 0x000b); *p++ = 3;
	*p++ = 0x04; *p++ = 0x01; *p++ = 0x00;		/* PCR 2, PCR 8 */
	if (tpm_cmd(fd, c, p, r, sizeof r))
		return 0;
	return h;
}

static void tpm_flush(int fd, unsigned h)
{
	unsigned char c[32], r[32], *p = c;

	be16(&p, 0x8001); be32(&p, 0); be32(&p, 0x165); be32(&p, h);
	tpm_cmd(fd, c, p, r, sizeof r);
}

/* close the door: after this PCR 8 no longer matches any sealed policy */
static int tpm_extend_pcr8(int fd)
{
	unsigned char c[128], r[64], *p = c;

	be16(&p, 0x8002); be32(&p, 0); be32(&p, 0x182);	/* PCR_Extend */
	be32(&p, 8);
	be32(&p, 9); be32(&p, 0x40000009); be16(&p, 0); *p++ = 0; be16(&p, 0);
	be32(&p, 1); be16(&p, 0x000b);
	memset(p, 0, 32);
	memcpy(p, "t480-early-init: key released", 29);
	p += 32;
	return tpm_cmd(fd, c, p, r, sizeof r);
}

/* ---- kernel keys ---- */

static long add_key(const char *type, const char *desc, const char *payload)
{
	return syscall(SYS_add_key, type, desc, payload, strlen(payload), KEY_SPEC_USER_KEYRING);
}

static char blob[8192], payload[9000];

static int load_trusted(const char *dir, int tpm)
{
	char path[128];
	unsigned h;
	long id;

	/* one-shot blob left by `t480-reseal --next-boot` before a firmware flash */
	snprintf(path, sizeof path, "%s/kmk.next", dir);
	if (slurp(path, blob, sizeof blob) > 0) {
		snprintf(payload, sizeof payload, "load %s keyhandle=" TPM_PARENT, blob);
		if (add_key("trusted", "t480-kmk", payload) >= 0) {
			say("master key from the one-shot blob (firmware changed, reseal pending)");
			return 0;
		}
	}
	snprintf(path, sizeof path, "%s/kmk.blob", dir);
	if (slurp(path, blob, sizeof blob) <= 0) {
		say("no %s", path);
		return -1;
	}
	h = tpm >= 0 ? tpm_policy_session(tpm) : 0;
	if (!h) {
		say("TPM policy session failed");
		return -1;
	}
	snprintf(payload, sizeof payload, "load %s keyhandle=" TPM_PARENT " policyhandle=0x%08x", blob, h);
	id = add_key("trusted", "t480-kmk", payload);
	if (id < 0) {
		say("TPM did not release the master key (errno %d): firmware or boot path changed", errno);
		tpm_flush(tpm, h);
		return -1;
	}
	return 0;
}

static int load_encrypted(const char *dir, const char *name)
{
	char path[128], desc[32];

	snprintf(path, sizeof path, "%s/%s.key", dir, name);
	if (slurp(path, blob, sizeof blob) <= 0)
		return -1;
	snprintf(payload, sizeof payload, "load %s", blob);
	snprintf(desc, sizeof desc, "t480-%s", name);
	if (add_key("encrypted", desc, payload) < 0) {
		say("encrypted key %s: errno %d", desc, errno);
		return -1;
	}
	return 0;
}

/* ---- device-mapper ---- */

static int dm_ioctl(int ctl, unsigned long cmd, struct dm_ioctl *io, const char *name)
{
	io->version[0] = 4;
	io->version[1] = 0;
	io->version[2] = 0;
	io->data_start = sizeof *io;
	if (!io->data_size)
		io->data_size = sizeof *io;
	strncpy(io->name, name, sizeof io->name - 1);
	return ioctl(ctl, cmd, io);
}

/* map `dev` as /dev/dm-N named `name` with the volume key t480-<name>; returns N or -1 */
static int dm_crypt(const char *dir, const char *name, const char *dev)
{
	static union { struct dm_ioctl io; char raw[2048]; } u;
	struct dm_target_spec *t;
	char path[128], conf[64], luks_uuid[41] = "", *params;
	unsigned long long bytes = 0, offset, ssize, len;
	int ctl, fd, mn, n;

	snprintf(path, sizeof path, "%s/%s.dm", dir, name);
	if (slurp(path, conf, sizeof conf) <= 0 || sscanf(conf, "%llu %llu", &offset, &ssize) != 2 ||
	    (ssize != 512 && ssize != 4096)) {
		say("bad %s", path);
		return -1;
	}
	fd = open(dev, O_RDONLY);
	if (fd < 0 || ioctl(fd, BLKGETSIZE64, &bytes)) {
		say("%s: errno %d", dev, errno);
		return -1;
	}
	pread(fd, luks_uuid, 40, 168);
	close(fd);
	len = bytes / 512;
	if (offset >= len)
		return -1;
	len = (len - offset) / (ssize / 512) * (ssize / 512);

	ctl = open("/dev/mapper/control", O_RDWR);
	if (ctl < 0) {
		say("no device-mapper: errno %d", errno);
		return -1;
	}
	memset(&u, 0, sizeof u);
	/* the uuid cryptsetup would give it, so `cryptsetup status` and udev know the device */
	n = snprintf(u.io.uuid, sizeof u.io.uuid, "CRYPT-LUKS2-");
	for (char *s = luks_uuid; *s && n < (int)sizeof u.io.uuid - 8; s++)
		if (*s != '-')
			u.io.uuid[n++] = *s;
	snprintf(u.io.uuid + n, sizeof u.io.uuid - n, "-%s", name);
	if (dm_ioctl(ctl, DM_DEV_CREATE, &u.io, name)) {
		say("dm create %s: errno %d", name, errno);
		close(ctl);
		return -1;
	}
	mn = minor(u.io.dev);

	memset(&u, 0, sizeof u);
	u.io.data_size = sizeof u;
	u.io.target_count = 1;
	t = (struct dm_target_spec *)(u.raw + sizeof u.io);
	t->sector_start = 0;
	t->length = len;
	strcpy(t->target_type, "crypt");
	params = (char *)(t + 1);
	n = snprintf(params, sizeof u - sizeof u.io - sizeof *t,
		     "aes-xts-plain64 :64:encrypted:t480-%s 0 %s %llu %d allow_discards no_read_workqueue no_write_workqueue",
		     name, dev, offset, ssize == 512 ? 3 : 4);
	if (ssize != 512)
		snprintf(params + n, 32, " sector_size:%llu", ssize);
	if (dm_ioctl(ctl, DM_TABLE_LOAD, &u.io, name)) {
		say("dm table %s: errno %d", name, errno);
		goto undo;
	}
	memset(&u, 0, sizeof u);
	if (dm_ioctl(ctl, DM_DEV_SUSPEND, &u.io, name)) {	/* no suspend flag: resume */
		say("dm resume %s: errno %d", name, errno);
		goto undo;
	}
	close(ctl);
	return mn;
undo:
	memset(&u, 0, sizeof u);
	dm_ioctl(ctl, DM_DEV_REMOVE, &u.io, name);
	close(ctl);
	return -1;
}

static int dm_minor_of(const char *name)
{
	static struct dm_ioctl io;
	int ctl = open("/dev/mapper/control", O_RDWR), r;

	memset(&io, 0, sizeof io);
	r = ctl < 0 ? -1 : dm_ioctl(ctl, DM_DEV_STATUS, &io, name);
	if (ctl >= 0)
		close(ctl);
	return r ? -1 : (int)minor(io.dev);
}

/* ---- passphrase fallback ---- */

static int read_passphrase(char *buf, int max)
{
	struct termios old, raw;
	int fd = open("/dev/console", O_RDWR), n = 0;
	char c;

	if (fd < 0)
		return -1;
	tcgetattr(fd, &old);
	raw = old;
	raw.c_lflag &= ~(ECHO | ICANON);
	raw.c_lflag |= ISIG;
	raw.c_cc[VMIN] = 1;
	raw.c_cc[VTIME] = 0;
	tcsetattr(fd, TCSANOW, &raw);
	while (n < max - 1 && read(fd, &c, 1) == 1 && c != '\n' && c != '\r') {
		if (c == 0x7f || c == 8) {
			if (n)
				n--;
		} else {
			buf[n++] = c;
		}
	}
	buf[n] = 0;
	tcsetattr(fd, TCSANOW, &old);
	write(fd, "\n", 1);
	close(fd);
	return n;
}

static int cryptsetup_open(const char *dev, const char *name, const char *pass)
{
	int p[2], st = -1;
	pid_t pid;

	if (pipe(p))
		return -1;
	pid = fork();
	if (!pid) {
		int nul = open("/dev/null", O_RDWR);

		dup2(p[0], 0);
		dup2(nul, 1);
		dup2(nul, 2);
		close(p[1]);
		execl(CRYPTSETUP, "cryptsetup", "open", "--type", "luks2", "--disable-locks", "--key-file", "-",
		      "--allow-discards", "--perf-no_read_workqueue", "--perf-no_write_workqueue",
		      dev, name, (char *)NULL);
		_exit(127);
	}
	close(p[0]);
	write(p[1], pass, strlen(pass));
	close(p[1]);
	if (pid < 0 || waitpid(pid, &st, 0) < 0)
		return -1;
	return WIFEXITED(st) ? WEXITSTATUS(st) : -1;
}

static void stop(int how)
{
	sync();
	reboot(how);
	for (;;)
		pause();
}

/* opens whatever is still closed with the passphrase; never returns without success */
static void passphrase_unlock(int need_root, int need_swap, int *root_minor, int *swap_minor)
{
	static char pass[256];

	tell("\n\n  T480: the TPM did not release the disk key.\n"
	     "  This is expected after a firmware change, a TPM reset or with the disk in\n"
	     "  another machine. Enter the LUKS recovery passphrase to continue;\n"
	     "  afterwards run `t480-reseal rekey` as root so the next boot is automatic again.\n"
	     "  An empty passphrase powers the machine off.\n\n");
	for (int try = 0; ; try++) {
		tell("  LUKS passphrase: ");
		if (read_passphrase(pass, sizeof pass) <= 0)
			stop(RB_POWER_OFF);
		if (need_root && *root_minor < 0 && !cryptsetup_open(P_ROOT, "root", pass))
			*root_minor = dm_minor_of("root");
		if (need_swap && *swap_minor < 0 && (!need_root || *root_minor >= 0) &&
		    !cryptsetup_open(P_SWAP, "swap", pass))
			*swap_minor = dm_minor_of("swap");
		if ((!need_root || *root_minor >= 0) && (!need_swap || *swap_minor >= 0 || try >= 2))
			break;
		tell("  That did not open the disk.\n");
		if (try >= 9) {
			tell("  Too many attempts, powering off.\n");
			nap_ms(3000);
			stop(RB_POWER_OFF);
		}
	}
	memset(pass, 0, sizeof pass);
}

/* ---- main ---- */

/* a bare word on the command line (t480.provision), not part of another word or value */
static int cmdline_flag(const char *cl, const char *word)
{
	const char *p = cl;
	int wl = strlen(word);

	while ((p = strstr(p, word))) {
		if ((p == cl || p[-1] == ' ') && (p[wl] == 0 || p[wl] == ' ' || p[wl] == '\n' || p[wl] == '='))
			return 1;
		p += wl;
	}
	return 0;
}

static void cmdline_arg(const char *cl, const char *key, char *out, int max, const char *dflt)
{
	const char *p = cl;
	int kl = strlen(key), n = 0;

	snprintf(out, max, "%s", dflt);
	while ((p = strstr(p, key))) {
		if (p == cl || p[-1] == ' ') {
			p += kl;
			while (*p && *p != ' ' && *p != '\n' && n < max - 1)
				out[n++] = *p++;
			out[n] = 0;
			return;
		}
		p += kl;
	}
}

int main(void)
{
	static char cl[4096], initp[256], res[32];
	const char *keydir = NULL, *rootdev = P_ROOT;
	int root_luks, swap_luks, tpm, root_minor = -1, swap_minor = -1, have_keys = 0, provision;
	double t0;

	mount("devtmpfs", "/dev", "devtmpfs", MS_NOSUID, "mode=0755");
	mount("proc", "/proc", "proc", MS_NOSUID | MS_NOEXEC | MS_NODEV, NULL);
	mount("sysfs", "/sys", "sysfs", MS_NOSUID | MS_NOEXEC | MS_NODEV, NULL);
	kmsg = open("/dev/kmsg", O_WRONLY);
	logfd = open("/dev/t480-early.log", O_WRONLY | O_CREAT | O_TRUNC, 0600);
	t0 = now();
	slurp("/proc/cmdline", cl, sizeof cl);

	if (wait_for(P_ROOT, 10000)) {
		tell("\n  T480: no disk (" P_ROOT "). Powering off in 10 s.\n");
		nap_ms(10000);
		stop(RB_POWER_OFF);
	}
	root_luks = is_luks2(P_ROOT);
	swap_luks = is_luks2(P_SWAP);

	/* the key directory, read without touching the file system (a hibernated system owns it) */
	if (!access(P_BOOT, F_OK) && !mount(P_BOOT, "/boot", "ext4", MS_RDONLY | MS_NOEXEC | MS_NOSUID | MS_NODEV, "noload"))
		keydir = "/boot/t480";
	else if (!root_luks && !mount(P_ROOT, "/boot", "ext4", MS_RDONLY | MS_NOEXEC | MS_NOSUID | MS_NODEV, "noload"))
		keydir = "/boot/boot/t480";

	/*
	 * The master key is released only to open a LUKS2 root. With a plain root the next
	 * code to run is that root's init, which nothing has measured or signed, and the key
	 * would sit in its keyring: a swapped-in plain disk next to the real /boot would get
	 * it (found by the 2026-10-09 audit). The one exception is provisioning, the check
	 * boot of the conversion on the still plain disk ("TPM key released" in the log):
	 * the kernel command line must carry t480.provision. The command line comes from
	 * the GRUB configuration that coreboot measures into PCR 2, so only an entry or the
	 * shell behind the GRUB password can add a word the default entry does not have.
	 * A plain root next to an encrypted swap therefore asks the passphrase for the swap.
	 */
	provision = cmdline_flag(cl, "t480.provision");

	/* one attempt at the TPM, then PCR 8 is extended whatever happened */
	wait_for("/dev/tpm0", 2000);
	tpm = open("/dev/tpm0", O_RDWR);
	if (keydir && !access(keydir, F_OK)) {
		if (root_luks || provision)
			have_keys = !load_trusted(keydir, tpm);
		else
			say("plain root: TPM key left sealed (no t480.provision on the command line)");
	}
	if (tpm < 0 || tpm_extend_pcr8(tpm)) {
		/* without the extend a later system could unseal: only go on if nothing was unsealed */
		say("PCR 8 extend FAILED");
		if (have_keys) {
			tell("\n  T480: TPM error after the key release. Powering off in 10 s.\n");
			nap_ms(10000);
			stop(RB_POWER_OFF);
		}
	}
	if (tpm >= 0)
		close(tpm);

	if (have_keys) {
		if (root_luks && !load_encrypted(keydir, "root"))
			root_minor = dm_crypt(keydir, "root", P_ROOT);
		if (swap_luks && !load_encrypted(keydir, "swap"))
			swap_minor = dm_crypt(keydir, "swap", P_SWAP);
	}
	if (keydir)
		umount("/boot");
	say("root %s, swap %s, TPM key %s, %d ms", root_luks ? (root_minor >= 0 ? "opened" : "LOCKED") : "plain",
	    swap_luks ? (swap_minor >= 0 ? "opened" : "LOCKED") : "plain", have_keys ? "released" : "not released",
	    (int)((now() - t0) * 1000));

	if ((root_luks && root_minor < 0) || (swap_luks && swap_minor < 0))
		passphrase_unlock(root_luks, swap_luks, &root_minor, &swap_minor);

	/* resume before any file system is mounted for writing; returns only if there is no image */
	if (swap_minor >= 0 && !strstr(cl, "noresume")) {
		struct stat st;
		char dmdev[32];

		snprintf(dmdev, sizeof dmdev, "/dev/dm-%d", swap_minor);
		if (!wait_for(dmdev, 2000) && !stat(dmdev, &st)) {
			snprintf(res, sizeof res, "%u:%u", major(st.st_rdev), minor(st.st_rdev));
			spit("/sys/power/resume", res);
		}
	}

	if (root_luks) {
		static char dmdev[32];

		snprintf(dmdev, sizeof dmdev, "/dev/dm-%d", root_minor);
		wait_for(dmdev, 2000);
		rootdev = dmdev;
	}
	for (int try = 0; mount(rootdev, "/newroot", "ext4", strstr(cl, " ro ") ? MS_RDONLY : 0, NULL); try++) {
		say("mount %s: errno %d", rootdev, errno);
		if (try >= 2) {
			tell("\n  T480: the root file system cannot be mounted. Powering off in 15 s.\n");
			nap_ms(15000);
			stop(RB_POWER_OFF);
		}
		nap_ms(500);
	}

	cmdline_arg(cl, "init=", initp, sizeof initp, "/sbin/init");
	say("switching to %s on %s, %d ms in the early init", initp, rootdev, (int)((now() - t0) * 1000));
	close(kmsg);
	close(logfd);
	umount2("/proc", MNT_DETACH);
	umount2("/sys", MNT_DETACH);
	if (mount("/dev", "/newroot/dev", NULL, MS_MOVE, NULL))
		mount("devtmpfs", "/newroot/dev", "devtmpfs", MS_NOSUID, "mode=0755");
	unlink("/init");
	unlink(CRYPTSETUP);
	if (chdir("/newroot") || mount(".", "/", NULL, MS_MOVE, NULL) || chroot(".") || chdir("/")) {
		tell("\n  T480: switch_root failed. Powering off in 15 s.\n");
		nap_ms(15000);
		stop(RB_POWER_OFF);
	}
	{
		int fd = open("/dev/console", O_RDWR);

		if (fd >= 0) {
			dup2(fd, 0);
			dup2(fd, 1);
			dup2(fd, 2);
			if (fd > 2)
				close(fd);
		}
	}
	execl(initp, initp, (char *)NULL);
	execl("/sbin/init", "/sbin/init", (char *)NULL);
	stop(RB_POWER_OFF);
	return 1;
}
