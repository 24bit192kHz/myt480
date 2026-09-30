#!/bin/bash
# Default acpi script that takes an entry for all actions

case "$1" in
    button/power)
        case "$2" in
            PBTN|PWRF)
                logger 'PowerButton pressed'
                ;;
            *)
                logger "ACPI action undefined: $2"
                ;;
        esac
        ;;
    button/sleep)
        case "$2" in
            SLPB|SBTN)
                logger 'SleepButton pressed'
                ;;
            *)
                logger "ACPI action undefined: $2"
                ;;
        esac
        ;;
    ac_adapter)
        case "$2" in
            AC|ACAD|ADP0)
                case "$4" in
                    00000000)
                        logger 'AC unpluged'
                        ;;
                    00000001)
                        logger 'AC pluged'
                        ;;
                esac
                ;;
            *)
                logger "ACPI action undefined: $2"
                ;;
        esac
        ;;
    battery)
        case "$2" in
            BAT0)
                case "$4" in
                    00000000)
                        logger 'Battery online'
                        ;;
                    00000001)
                        logger 'Battery offline'
                        ;;
                esac
                ;;
            CPU0)
                ;;
            *)  logger "ACPI action undefined: $2" ;;
        esac
        ;;
    # Stock BIOS reports these as video/brightnessup BRTUP ... and leaves the
    # change to the OS. coreboot changes the brightness itself, so skip there
    # or every press would step twice.
    video|video/*)
        [ "$(cat /sys/class/dmi/id/bios_vendor 2>/dev/null)" = coreboot ] && exit 0
        case "$2" in
            brightnessdown|BRTDN)
                brightnessctl set 5%- 2>/dev/null
                ;;
            brightnessup|BRTUP)
                brightnessctl set 5%+ 2>/dev/null
                ;;
            *)
                logger "ACPI video undefined: $2"
                ;;
        esac
        ;;
    # button/lid intentionally unhandled here: elogind owns it
    # (HandleLidSwitch=suspend-then-hibernate in
    # /etc/elogind/logind.conf.d/10-lid-suspend.conf). acpid must NEVER
    # suspend on lid; dual ownership caused missed/double suspends.
    button/lid)
        logger 'LID event ignored by acpid (handled by elogind)'
        ;;
    *)
        logger "ACPI group/action undefined: $1 / $2"
        ;;
esac

# vim:set ts=4 sw=4 ft=sh et:
