/* hijri.c — Hijri dates for the chadwm bar and calendar (calendar.sh).
 * Umm al-Qura, the official Saudi calendar: ICU's islamic-umalqura table
 * (1300-1600 AH). Civil use: the day changes at midnight with the Gregorian
 * one, not at sunset.
 * The Hijri side is Arabic (ar_SA month names, Arabic-Indic digits).
 *   hijri            today        "٢٠ ربيع الآخر ١٤٤٨ هـ"
 *   hijri short      today        "٢٠ ربيع الآخر"
 *   hijri cal [N]    the Gregorian month N months from now (default 0):
 *                    line 1 = title, then Pango markup, Sunday first, the
 *                    Hijri day under each date; today underlined (both), the
 *                    1st of a Hijri month bold. Arabic lines use $HIJRI_FONT
 *                    (default Noto Kufi Arabic); the Hijri digit rows use
 *                    DejaVu Sans Mono, whose ٠-٩ have JetBrains Mono's width,
 *                    so the columns stay aligned.
 *   hijri tip        this month for the chadwm hover popup (bartip.c codes:
 *                    ^>N^ right-aligns a cell at column N), Gregorian row
 *                    then Hijri row; colours from $TIP_ACC, $TIP_DIM and
 *                    $TIP_BG (today = $TIP_BG on $TIP_ACC)
 *   hijri days Y M   one line per day of Gregorian month M (1-12) of Y:
 *                    "D HD HM HY<TAB>Arabic Hijri month name" (calendar app)
 * build: cc -O2 -o ~/.local/bin/hijri hijri.c $(pkg-config --cflags --libs icu-i18n icu-uc)
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unicode/ucal.h>
#include <unicode/udat.h>
#include <unicode/ustring.h>

#define DAY (24.0 * 60 * 60 * 1000)

static UErrorCode err = U_ZERO_ERROR;

static void die(const char *what)
{
	fprintf(stderr, "hijri: %s: %s\n", what, u_errorName(err));
	exit(1);
}

static UCalendar *cal(const char *locale)
{
	UCalendar *c = ucal_open(NULL, -1, locale, UCAL_DEFAULT, &err);

	if (U_FAILURE(err))
		die(locale);
	return c;
}

/* ICU pattern -> UTF-8, e.g. "MMMM y" in the given calendar's locale */
static const char *fmt(const char *locale, const char *pattern, UDate t)
{
	static char out[4][128];
	static int n;
	UChar pat[64], buf[128];
	UDateFormat *f;
	char *o = out[n++ % 4];

	u_uastrcpy(pat, pattern);
	f = udat_open(UDAT_PATTERN, UDAT_PATTERN, locale, NULL, -1, pat, -1, &err);
	if (U_FAILURE(err))
		die(pattern);
	udat_format(f, t, buf, 128, NULL, &err);
	u_strToUTF8(o, 128, NULL, buf, -1, &err);
	udat_close(f);
	if (U_FAILURE(err))
		die("format");
	return o;
}

#define HL "ar_SA@calendar=islamic-umalqura;numbers=arab"
#define GL "en@calendar=gregorian"
#define MONO "DejaVu Sans Mono"

/* n in Arabic-Indic digits (U+0660..U+0669), right-aligned in w cells */
static const char *ar_num(int n, int w)
{
	static char out[4][32];
	static int k;
	char d[12], *o = out[k++ % 4], *p = o;
	int len = snprintf(d, sizeof d, "%d", n);

	for (; w > len; w--)
		*p++ = ' ';
	for (int i = 0; i < len; i++) {
		*p++ = '\xd9';
		*p++ = (char)(0xa0 + d[i] - '0');
	}
	*p = 0;
	return o;
}

static void month(int offset)
{
	UCalendar *g = cal(GL), *h = cal(HL);
	UDate first, today = ucal_getNow();
	int ty, tm, td, days, wd, d, i, row;
	int hd[32];
	char hmonths[256];
	const char *af = getenv("HIJRI_FONT");

	if (!af || !*af)
		af = "Noto Kufi Arabic";

	ucal_setMillis(g, today, &err);
	ty = ucal_get(g, UCAL_YEAR, &err);
	tm = ucal_get(g, UCAL_MONTH, &err);
	td = ucal_get(g, UCAL_DATE, &err);
	ucal_setDateTime(g, ty, tm, 1, 12, 0, 0, &err);
	ucal_add(g, UCAL_MONTH, offset, &err);
	first = ucal_getMillis(g, &err);
	days = ucal_getLimit(g, UCAL_DATE, UCAL_ACTUAL_MAXIMUM, &err);
	wd = ucal_get(g, UCAL_DAY_OF_WEEK, &err) - UCAL_SUNDAY;	/* 0 = Sunday */
	if (U_FAILURE(err))
		die("month");

	for (d = 1; d <= days; d++) {
		ucal_setMillis(h, first + (d - 1) * DAY, &err);
		hd[d] = ucal_get(h, UCAL_DATE, &err);
	}
	/* Hijri months this Gregorian month touches: "ربيع الآخر – جمادى الأولى ١٤٤٨ هـ" */
	snprintf(hmonths, sizeof hmonths, "%s", fmt(HL, "MMMM", first));
	for (d = 2; d <= days; d++)
		if (hd[d] == 1) {
			const char *y0 = fmt(HL, "y", first);
			const char *y1 = fmt(HL, "y", first + (d - 1) * DAY);
			size_t l = strlen(hmonths);

			if (strcmp(y0, y1))
				snprintf(hmonths + l, sizeof hmonths - l, " %s", y0);
			l = strlen(hmonths);
			snprintf(hmonths + l, sizeof hmonths - l, " – %s",
				 fmt(HL, "MMMM", first + (d - 1) * DAY));
		}
	{
		size_t l = strlen(hmonths);
		snprintf(hmonths + l, sizeof hmonths - l, " %s",
			 fmt(HL, "y G", first + (days - 1) * DAY));
	}

	printf("%s\n", fmt(GL, "MMMM y", first));
	if (offset == 0)
		printf("<b>%s</b>\n<span font_family=\"%s\" weight=\"bold\">%s</span>\n",
		       fmt(GL, "EEEE d MMMM y", today), af, fmt(HL, "EEEE d MMMM y G", today));
	printf("<span font_family=\"%s\">%s</span>\n\n", af, hmonths);
	printf("<b> Sun Mon Tue Wed Thu Fri Sat</b>\n");
	for (row = 1 - wd; row <= days; row += 7) {
		for (i = row; i < row + 7; i++) {	/* Gregorian */
			if (i < 1 || i > days)
				printf("    ");
			else if (offset == 0 && i == td)	/* pad outside the underline */
				printf("%*s<span weight=\"bold\" underline=\"single\">%d</span>",
				       i < 10 ? 3 : 2, "", i);
			else
				printf("%4d", i);
		}
		putchar('\n');
		printf("<span font_family=\"" MONO "\">");
		for (i = row; i < row + 7; i++) {	/* Hijri under it */
			/* LRM before each cell: Pango takes a line of Arabic-Indic
			   digits as right-to-left and would mirror the week */
			printf("\u200e");
			if (i < 1 || i > days)
				printf("    ");
			else if (offset == 0 && i == td)	/* pad outside the underline */
				printf("%*s<span weight=\"bold\" underline=\"single\">%s</span>",
				       hd[i] < 10 ? 3 : 2, "", ar_num(hd[i], 0));
			else if (hd[i] == 1)
				printf("<span weight=\"bold\">%s</span>", ar_num(hd[i], 4));
			else
				printf("<span fgalpha=\"60%%\">%s</span>", ar_num(hd[i], 4));
		}
		printf("</span>\n");
	}
	ucal_close(g);
	ucal_close(h);
}

static const char *env(const char *name, const char *def)
{
	const char *v = getenv(name);

	return v && *v ? v : def;
}

static void tip(void)
{
	UCalendar *g = cal(GL), *h = cal(HL);
	UDate first, today = ucal_getNow();
	const char *acc = env("TIP_ACC", "#cfcfcf"), *dim = env("TIP_DIM", "#909090"),
		   *bg = env("TIP_BG", "#101010");
	/* Arabic one-letter weekdays, Sunday first; the grid runs right to left */
	const char *wk[] = { "ح", "ن", "ث", "ر", "خ", "ج", "س" };
	int td, days, wd, d, i, row;
	int hd[32];

	ucal_setMillis(g, today, &err);
	td = ucal_get(g, UCAL_DATE, &err);
	ucal_set(g, UCAL_DATE, 1);
	ucal_set(g, UCAL_HOUR_OF_DAY, 12);
	first = ucal_getMillis(g, &err);
	days = ucal_getLimit(g, UCAL_DATE, UCAL_ACTUAL_MAXIMUM, &err);
	wd = ucal_get(g, UCAL_DAY_OF_WEEK, &err) - UCAL_SUNDAY;
	if (U_FAILURE(err))
		die("tip");
	for (d = 1; d <= days; d++) {
		ucal_setMillis(h, first + (d - 1) * DAY, &err);
		hd[d] = ucal_get(h, UCAL_DATE, &err);
	}

	printf("^c%s^%s^d^\n", acc, fmt(GL, "EEEE d MMMM y", today));
	printf("^c%s^%s^d^\n\n", acc, fmt(HL, "EEEE d MMMM y G", today));
	/* column of weekday k (0 = Sunday) ends at COL(k): Sunday rightmost.
	   Cells go out left to right (Saturday first): bartip only moves on. */
#define COL(k) (4 * (7 - (k)))
	for (i = 6; i >= 0; i--)
		printf("^c%s^^>%d^%s", dim, COL(i), wk[i]);
	printf("^d^\n");
	for (row = 1 - wd; row <= days; row += 7) {
		for (i = row + 6; i >= row; i--) {
			if (i < 1 || i > days)
				continue;
			if (i == td)
				printf("^>%d^^b%s^^c%s^%2d^d^", COL(i - row), acc, bg, i);
			else
				printf("^>%d^%d", COL(i - row), i);
		}
		printf("\n");
		for (i = row + 6; i >= row; i--) {
			if (i < 1 || i > days)
				continue;
			printf("^>%d^^c%s^%s^d^", COL(i - row),
			       i == td ? acc : hd[i] == 1 ? "" : dim, ar_num(hd[i], 0));
		}
		printf("\n");
	}
#undef COL
	ucal_close(g);
	ucal_close(h);
}

static void days(int y, int m)
{
	UCalendar *g = cal(GL), *h = cal(HL);
	int d, n;

	ucal_setDateTime(g, y, m - 1, 1, 12, 0, 0, &err);
	n = ucal_getLimit(g, UCAL_DATE, UCAL_ACTUAL_MAXIMUM, &err);
	for (d = 1; d <= n; d++) {
		UDate t;

		ucal_setDateTime(g, y, m - 1, d, 12, 0, 0, &err);
		t = ucal_getMillis(g, &err);
		ucal_setMillis(h, t, &err);
		if (U_FAILURE(err))
			die("days");
		printf("%d %d %d %d\t%s\n", d, ucal_get(h, UCAL_DATE, &err),
		       ucal_get(h, UCAL_MONTH, &err) + 1, ucal_get(h, UCAL_EXTENDED_YEAR, &err),
		       fmt(HL, "MMMM", t));
	}
	ucal_close(g);
	ucal_close(h);
}

int main(int argc, char **argv)
{
	UDate now = ucal_getNow();

	if (argc < 2) {
		puts(fmt(HL, "d MMMM y G", now));
	} else if (!strcmp(argv[1], "short")) {
		puts(fmt(HL, "d MMMM", now));
	} else if (!strcmp(argv[1], "days") && argc > 3) {
		days(atoi(argv[2]), atoi(argv[3]));
	} else if (!strcmp(argv[1], "tip")) {
		tip();
	} else if (!strcmp(argv[1], "cal")) {
		month(argc > 2 ? atoi(argv[2]) : 0);
	} else {
		fputs("usage: hijri [short | tip | days Y M | cal [MONTHS]]\n", stderr);
		return 2;
	}
	return 0;
}
