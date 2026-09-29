include config.mk

slock-ly: slock.c config.h
	$(CC) $(CFLAGS) -o $@ slock.c $(LDFLAGS) $(LIBS)

install: slock-ly
	install -D -m 4755 -o root -g root slock-ly $(DESTDIR)$(PREFIX)/bin/slock-ly

clean:
	rm -f slock-ly
