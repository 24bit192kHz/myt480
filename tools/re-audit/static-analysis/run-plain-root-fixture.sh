#!/bin/sh
# Actual-source ordering fixture only. Never invokes early-init without mocks.
set -eu
umask 077

fixture_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
source_path="$fixture_dir/../../../kernel/early-init/init.c"
expected_hash=59840d7b9a8a8485be5803ee7ef48c522511ea81ffcff2694e5c5c6ad8894628

for tool in cc nm sha256sum mktemp cp mkdir rm awk; do
    command -v "$tool" >/dev/null 2>&1 || {
        printf 'Missing required workstation tool: %s\n' "$tool" >&2
        exit 1
    }
done

fixture_work=$(mktemp -d "${TMPDIR:-/tmp}/t480-plain-root.XXXXXX")
trap 'rm -rf -- "$fixture_work"' 0
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$fixture_work/kernel/early-init" "$fixture_work/tools/re-audit/static-analysis"
cp -- "$source_path" "$fixture_work/kernel/early-init/init.c"
cp -- "$fixture_dir/plain-root-harness.c" "$fixture_work/tools/re-audit/static-analysis/plain-root-harness.c"
actual_hash=$(sha256sum "$fixture_work/kernel/early-init/init.c")
actual_hash=${actual_hash%% *}
if [ "$actual_hash" != "$expected_hash" ]; then
    printf '%s\n' 'Refusing to compile or run: init.c differs from the reviewed exact source.' >&2
    exit 1
fi

wrapped='open access read write close syscall mount umount umount2 unlink chdir chroot execl reboot dup2 ioctl pread tcgetattr tcsetattr pipe fork waitpid sync pause _exit stat clock_gettime nanosleep'
cc -std=c11 -O0 -Wall -Wextra -fno-builtin -fno-lto \
    -U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0 -DT480_OFFLINE_FIXTURE_WRAPPERS=1 \
    -c "$fixture_work/tools/re-audit/static-analysis/plain-root-harness.c" \
    -o "$fixture_work/fixture.o"

# Reject additional external operations before running the compiled fixture.
# Harmless libc formatting/memory/error helpers remain real; all source-facing
# device/file/process/privileged operations must use the reviewed wrappers.
allowed="$wrapped getenv __assert_fail __errno_location __stack_chk_fail _setjmp setjmp longjmp abort puts fputs stderr memcmp memcpy memset snprintf vsnprintf strlen strcmp strncmp strcpy strncpy strstr sscanf __isoc99_sscanf __isoc23_sscanf gnu_dev_major gnu_dev_minor"
for symbol in $(nm -u "$fixture_work/fixture.o" | awk '{print $NF}'); do
    case " $allowed " in
        *" $symbol "*) ;;
        *) printf 'Refusing unreviewed external symbol: %s\n' "$symbol" >&2; exit 1 ;;
    esac
done

set --
for symbol in $wrapped; do
    set -- "$@" "-Wl,--wrap=$symbol"
done
cc -fno-lto "$fixture_work/fixture.o" "$@" -o "$fixture_work/fixture"
for symbol in $(nm -u "$fixture_work/fixture" | awk '{print $NF}'); do
    symbol=${symbol%%@*}
    case " $wrapped " in
        *" $symbol "*) printf 'Refusing unresolved real operation: %s\n' "$symbol" >&2; exit 1 ;;
    esac
done
"$fixture_work/fixture"
T480_FIXTURE_PROVISION=1 "$fixture_work/fixture"
