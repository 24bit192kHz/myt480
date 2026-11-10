# Verification for the whole kit: `make check`. Nothing here touches hardware or needs root.
# Tests build in temporary files; the tree stays clean.
PY = PYTHONDONTWRITEBYTECODE=1 python3

.PHONY: check drift check-shell check-re-audit check-gpu-power check-thermald check-early-init

check: check-shell check-re-audit check-gpu-power check-thermald check-early-init
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
	@for t in src/thermald-t480/tests/*.c; do \
		b=$$(mktemp) && cc -O2 -Wall -o "$$b" "$$t" && "$$b" >/dev/null 2>&1; rc=$$?; rm -f "$$b"; \
		[ $$rc -eq 0 ] || { echo "check-thermald: $$t failed"; exit 1; }; \
	done
	@echo "check-thermald: ok"

# the early init's command-line parsing only; the boot path itself is rehearsed in QEMU
# (kernel/early-init/qemu/go.sh)
check-early-init:
	@b=$$(mktemp) && cc -O2 -Wall -o "$$b" kernel/early-init/tests/cmdline.c && "$$b" >/dev/null; \
	rc=$$?; rm -f "$$b"; [ $$rc -eq 0 ] && echo "check-early-init: ok"; exit $$rc

# Does the kit still describe the laptop? Not part of `check`: it needs the laptop.
#   make drift REACH="ssh HOST ssh root@localhost"     (no REACH: this machine is the laptop)
drift:
	@tools/kit-drift/kit-drift $(if $(REACH),-- $(REACH))
