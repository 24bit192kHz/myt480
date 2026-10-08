CC = musl-gcc
CFLAGS = -Os -march=skylake -mtune=skylake -static -s -Wall -Wextra -fno-asynchronous-unwind-tables -fno-stack-protector -ffunction-sections -fdata-sections
LDFLAGS = -Wl,--gc-sections -Wl,--build-id=none -Wl,-z,noseparate-code -Wl,-z,norelro -Wl,--hash-style=gnu

thermald-t480: thermald.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<

clean:
	rm -f thermald-t480
