# tty1 autologin -> startx immediately, locker starts first inside X.
if [ -z "$DISPLAY" ] && [ "$(tty)" = /dev/tty1 ]; then
  # i915 loads in parallel (udev) ~1 s after this login. X needs the card
  # *and* elogind knowing it: an earlier X only sees the boot framebuffer or
  # gets "failed to take device" from elogind, then "no screens found".
  # Wait (max 5 s) until udev has tagged the i915 card for the seat.
  # (N): an empty glob is not an error -- while i915 replaces simpledrm there
  # is briefly no card at all, and a zsh NOMATCH error would abort this file.
  wait_i915() {
    local d
    for _ in {1..250}; do
      for d in /sys/class/drm/card[0-9](N); do
        [[ "${$(readlink -f $d/device/driver 2>/dev/null)##*/}" == i915 &&
           -e /dev/dri/${d##*/} && -e /run/udev/tags/master-of-seat/c$(<$d/dev) ]] && return 0
      done
      sleep 0.02
    done
    return 1
  }
  wait_i915
  # a start that dies within 5 s is a failed start (not a logout): retry
  # right away instead of ending the login and waiting for the respawn
  for _ in 1 2 3; do
    t=$SECONDS
    startx
    (( SECONDS - t > 5 )) && break
    sleep 0.3
  done
  exit
fi
