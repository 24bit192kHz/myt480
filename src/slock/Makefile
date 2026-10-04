include config.mk

slock: slock.c config.h
	$(CC) $(CFLAGS) -o $@ slock.c $(LDFLAGS) $(LIBS)

install: slock
	install -D -m 4755 -o root -g root slock $(DESTDIR)$(PREFIX)/bin/slock
	install -D -m 0755 -o root -g root slock-keyring $(DESTDIR)$(PREFIX)/libexec/slock-keyring

clean:
	rm -f slock
