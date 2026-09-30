/* clipwatch — print one line each time the CLIPBOARD selection changes owner
 * (XFixes, event-driven, no polling). Feeds `rofi-clip.sh daemon`.
 * Exits when the X connection goes away.
 * build: cc -O2 -o ~/.local/bin/clipwatch clipwatch.c -lXfixes -lX11
 */
#include <stdio.h>
#include <X11/Xlib.h>
#include <X11/extensions/Xfixes.h>

int
main(void)
{
	Display *dpy;
	XEvent e;
	int evbase, errbase;

	if (!(dpy = XOpenDisplay(NULL))) {
		fputs("clipwatch: cannot open display\n", stderr);
		return 1;
	}
	if (!XFixesQueryExtension(dpy, &evbase, &errbase)) {
		fputs("clipwatch: no XFixes\n", stderr);
		return 1;
	}
	XFixesSelectSelectionInput(dpy, DefaultRootWindow(dpy),
	    XInternAtom(dpy, "CLIPBOARD", False),
	    XFixesSetSelectionOwnerNotifyMask);
	for (;;) {
		XNextEvent(dpy, &e);
		if (e.type == evbase + XFixesSelectionNotify) {
			putchar('\n');
			if (fflush(stdout) == EOF)
				return 0;
		}
	}
}
