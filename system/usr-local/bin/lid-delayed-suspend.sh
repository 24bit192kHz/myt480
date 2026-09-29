#!/bin/sh
# acpid lid handler: 60s grace, suspend-then-hibernate only if still closed.
# Reopen cancels: act only if lid STILL closed 60s after close event.
case "$*" in
  *close*) ;;
  *) logger -t lid-handler "lid event ignored: $*"; exit 0 ;;
esac
logger -t lid-handler "lid closed, 60s grace started"
(
  sleep 60
  LID_STATE=$(cat /proc/acpi/button/lid/*/state 2>/dev/null | awk '{print $2}' | head -n1)
  logger -t lid-handler "60s elapsed, lid state=$LID_STATE"
  if [ "$LID_STATE" = closed ]; then
    logger -t lid-handler "lid still closed -> suspend-then-hibernate"
    /usr/bin/loginctl suspend-then-hibernate
  else
    logger -t lid-handler "lid reopened, cancel suspend"
  fi
) &
exit 0
