/* The early init's reading of the kernel command line. Only the parsing functions run:
 * no device, TPM or mount is touched (main is renamed and never called). */
#define main early_init_main
#include "../init.c"
#undef main
#include <assert.h>

static const char *params(const char *cl)
{
	i915_cmdline_params(cl);
	return i915_params;
}

static const char *arg(const char *cl, const char *key, const char *dflt)
{
	static char out[64];

	cmdline_arg(cl, key, out, sizeof out, dflt);
	return out;
}

int main(void)
{
	/* ro / rw: whole words, the last one decides, a trailing word counts */
	assert(!cmdline_ro(""));
	assert(!cmdline_ro("root=/dev/x rw quiet"));
	assert(cmdline_ro("root=/dev/x ro quiet"));
	assert(cmdline_ro("root=/dev/x quiet ro"));
	assert(cmdline_ro("root=/dev/x quiet ro\n"));
	assert(cmdline_ro("ro"));
	assert(!cmdline_ro("ro quiet rw"));
	assert(cmdline_ro("rw quiet ro"));
	assert(!cmdline_ro("rootwait root=/dev/x rootflags=ro intro foo.ro=1 zero"));

	/* a flag is a bare word, never part of another word or of a value */
	assert(cmdline_flag("a t480.provision b", "t480.provision"));
	assert(cmdline_flag("t480.provision", "t480.provision"));
	assert(cmdline_flag("a t480.provision\n", "t480.provision"));
	assert(!cmdline_flag("a xt480.provision b", "t480.provision"));
	assert(!cmdline_flag("a t480.provisioning b", "t480.provision"));
	assert(!cmdline_flag("a foo=t480.provision b", "t480.provision"));

	assert(!strcmp(arg("quiet init=/usr/local/sbin/t480-init rw", "init=", "/sbin/init"), "/usr/local/sbin/t480-init"));
	assert(!strcmp(arg("quiet rw", "init=", "/sbin/init"), "/sbin/init"));
	assert(!strcmp(arg("rdinit=/x quiet", "init=", "/sbin/init"), "/sbin/init"));

	/* i915.name=value words become module parameters, as modprobe would pass them */
	assert(!strcmp(params("i915.enable_gvt=1 quiet"), "enable_gvt=1"));
	assert(!strcmp(params("quiet i915.enable_gvt=1 i915.enable_fbc=0\n"), "enable_gvt=1 enable_fbc=0"));
	assert(!strcmp(params("quiet xi915.enable_gvt=1 foo=i915.x=1"), ""));
	assert(!strcmp(params("quiet"), ""));
	puts("PASS: ro/rw, bare flags, init=, i915 module parameters");
	return 0;
}
