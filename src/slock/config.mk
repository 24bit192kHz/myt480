VERSION = 2.0
PREFIX = /usr/local
PKGS = x11 xrandr xrender xft
CFLAGS = -std=c99 -Os -Wall -Wextra $(shell pkg-config --cflags $(PKGS))
LDFLAGS = -s -Wl,--as-needed
LIBS = $(shell pkg-config --libs $(PKGS)) -lpam -lpthread
