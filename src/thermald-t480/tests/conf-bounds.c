/* Configuration values are bounded before they can reach an MSR or sysfs.
 * Only set_key() runs; no hardware is touched. */
#define _GNU_SOURCE
#include <assert.h>
#include <stdio.h>
#include <string.h>
#define main thermald_program_main
#include "../thermald.c"
#undef main

static long long *field(struct conf *c, const char *k)
{
    if (!strcmp(k, "icc_core_ac")) return &c->ac.icc[0];
    if (!strcmp(k, "icc_gpu_batt")) return &c->batt.icc[1];
    if (!strcmp(k, "ac_pl2_s")) return &c->ac.pl2_s;
    if (!strcmp(k, "batt_pl1_s")) return &c->batt.pl1_s;
    if (!strcmp(k, "save_igpu_mhz")) return &c->plan[PLAN_SAVE].igpu_mhz;
    if (!strcmp(k, "perf_turbo")) return &c->plan[PLAN_PERF].turbo;
    if (!strcmp(k, "ac_uv_core")) return &c->ac.uv[PL_CORE];
    if (!strcmp(k, "batt_pl1_w")) return &c->batt.pl1_w;
    if (!strcmp(k, "ac_trip")) return &c->ac.trip;
    if (!strcmp(k, "save_pl1_w")) return &c->plan[PLAN_SAVE].pl1_w;
    assert(!"unknown key in the test table");
    return 0;
}
/* a good value is taken, every bad one leaves it in place */
static void check(const char *k, const char *good, long long want, const char *const *bad)
{
    struct conf c;
    conf_defaults(&c);
    set_key(&c, k, good);
    if (*field(&c, k) != want) { printf("FAIL: %s %s -> %lld, want %lld\n", k, good, *field(&c, k), want); assert(0); }
    for (; *bad; bad++) {
        set_key(&c, k, *bad);
        if (*field(&c, k) != want) { printf("FAIL: %s accepted %s\n", k, *bad); assert(0); }
    }
}
int main(void)
{
    static const char *const icc[] = { "-64", "0", "0.5", "256", "640", "99999999", 0 };
    static const char *const win[] = { "-1", "0", "0.0001", "129", "3600", 0 };
    static const char *const mhz[] = { "-700", "0", "99", "2001", "70000", 0 };
    static const char *const turbo[] = { "-1", "2", "0.5", "10", 0 };
    static const char *const uv[] = { "5", "105", "-251", "-1050", 0 };
    static const char *const watt[] = { "-15", "0", "0.5", "201", 0 };
    static const char *const trip[] = { "39", "106", "-95", "950", 0 };
    struct conf c;

    check("icc_core_ac", "64", 64000, icc);
    check("icc_gpu_batt", "24.25", 24250, icc);
    check("ac_pl2_s", "0.002", 2, win);
    check("batt_pl1_s", "28", 28000, win);
    check("save_igpu_mhz", "700", 700000, mhz);
    check("perf_turbo", "0", 0, turbo);
    check("ac_uv_core", "-105", -105000, uv);
    check("batt_pl1_w", "15", 15000, watt);
    check("save_pl1_w", "10", 10000, watt);
    check("ac_trip", "95", 95000, trip);

    /* the largest accepted current limit still fits the 10-bit field of the mailbox word */
    conf_defaults(&c);
    set_key(&c, "icc_core_ac", "255");
    assert((icc_encode(0, c.ac.icc[0]) & ~(OC_CMD_ICC_SET | 0x3ffULL)) == 0);
    /* what a negative value used to do: it reached the command bits */
    assert((icc_encode(0, -64000) & ~(OC_CMD_ICC_SET | 0x3ffULL)) != 0);
    puts("PASS: current limits, time windows, iGPU clock, turbo, undervolt, power limits and trip points are bounded");
    return 0;
}
