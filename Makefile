# Verification for the whole kit: `make check`. Nothing here touches hardware or needs root.
# Tests build in temporary files; the tree stays clean.
PY = PYTHONDONTWRITEBYTECODE=1 python3

.PHONY: check check-shell check-re-audit check-gpu-power check-thermald

check: check-shell check-re-audit check-gpu-power check-thermald
	@echo "make check: all passed"

# syntax only: every tracked file whose first line names sh, bash or dash
check-shell:
	@git ls-files | while read -r f; do \
		[ -f "$$f" ] || continue; \
		case "$$(head -c 64 "$$f" | head -n 1)" in \
			'#!'*bash*) bash -n "$$f" || exit 1 ;; \
			'#!'*/sh*|'#!'*' sh'*|'#!'*dash*) sh -n "$$f" || exit 1 ;; \
		esac; \
	done
	@echo "check-shell: ok"

check-re-audit:
	$(PY) -m unittest discover -s tools/re-audit/tests

check-gpu-power:
	@for t in src/gpu-power/tests/*.py; do echo "== $$t"; $(PY) "$$t" || exit 1; done

check-thermald:
	@b=$$(mktemp) && cc -O2 -Wall -o "$$b" src/thermald-t480/tests/fan-control.c && "$$b" >/dev/null; \
	rc=$$?; rm -f "$$b"; [ $$rc -eq 0 ] && echo "check-thermald: ok"; exit $$rc
