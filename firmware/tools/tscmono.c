#include <stdio.h>
#include <time.h>
#include <cpuid.h>
#include <x86intrin.h>
int main(void){ unsigned a,b,c,d; __cpuid(0x15,a,b,c,d); double hz = 24e6*(double)b/(double)a;
 struct timespec r; unsigned long long t1=__rdtsc(); clock_gettime(CLOCK_MONOTONIC_RAW,&r); unsigned long long t2=__rdtsc();
 double tsc=(t1+t2)/2.0/hz, raw=r.tv_sec+r.tv_nsec/1e9;
 printf("cpuid15 %u/%u crystal=%u => tsc %.3f MHz; tsc_s=%.4f raw_s=%.4f => kernel clock zero at %.4f s after reset\n",b,a,c,hz/1e6,tsc,raw,tsc-raw); return 0;}
