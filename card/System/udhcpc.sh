#!/bin/sh
# udhcpc's event script, for systems without one of their own: puts the
# lease's address, route and name servers on the interface.
SD="${SLOT_ROOT:-/mnt/SDCARD}"
LOG="$SD/slot-net.log"
RESOLV="/etc/resolv.conf"

log() {
	echo "$(date '+%Y-%m-%d %H:%M:%S') dhcp: $*" >> "$LOG" 2>/dev/null
}

case "$1" in
deconfig)
	ip addr flush dev "$interface" 2>/dev/null
	ip link set "$interface" up 2>/dev/null
	;;
bound | renew)
	ip addr flush dev "$interface" 2>/dev/null
	ip addr add "$ip/${mask:-24}" dev "$interface" 2>/dev/null ||
		ifconfig "$interface" "$ip" netmask "${subnet:-255.255.255.0}" 2>/dev/null
	if [ -n "${router:-}" ]; then
		ip route del default dev "$interface" 2>/dev/null
		for r in $router; do
			ip route add default via "$r" dev "$interface" 2>/dev/null && break
		done
	fi
	if [ -n "${dns:-}" ]; then
		{
			[ -n "${domain:-}" ] && echo "search $domain"
			for d in $dns; do echo "nameserver $d"; done
		} > "$RESOLV.slot" 2>/dev/null
		cat "$RESOLV.slot" > "$RESOLV" 2>/dev/null || log "cannot write $RESOLV"
		rm -f "$RESOLV.slot"
	fi
	log "$1 $ip via ${router:-none} dns ${dns:-none}"
	;;
leasefail | nak)
	log "$1"
	;;
esac
exit 0
