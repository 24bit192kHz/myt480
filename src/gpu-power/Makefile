CFLAGS ?= -O2 -Wall -Wextra -std=c99

gpu-power: gpu-power.c
	$(CC) $(CFLAGS) -o $@ gpu-power.c

install: gpu-power
	install -D -m 4755 -o root -g root gpu-power $(DESTDIR)/usr/local/bin/gpu-power
	install -D -m 755 -o root -g root prime-run $(DESTDIR)/usr/local/bin/prime-run
	install -D -m 755 -o root -g root igpu-run $(DESTDIR)/usr/local/bin/igpu-run

clean:
	rm -f gpu-power

install-policy:
	install -D -m 755 -o root -g root gpu-power-hook $(DESTDIR)/usr/local/bin/gpu-power-hook
	install -D -m 644 -o root -g root 90-gpu-power.rules $(DESTDIR)/etc/udev/rules.d/90-gpu-power.rules
