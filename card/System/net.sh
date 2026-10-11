#!/bin/sh
# Wi-Fi as a client of the user's own network, on TrimUI's stock firmware
# and on the H700 handhelds alike.
#   net.sh up               turn Wi-Fi on and keep it on across link play
#   net.sh down             turn it off
#   net.sh pause            let go of wlan0 for link play, remembering it was on
#   net.sh resume           take wlan0 back if it was on before the pause
#   net.sh ip               print wlan0's address, if it has one
#   net.sh scan             print the networks in range, one per line:
#                           signal<TAB>flags<TAB>ssid
#   net.sh join             join NET_SSID with NET_PSK (empty for an open
#                           network), remembered for next time
#   net.sh ssid             print the network wlan0 is on, if any
# Networks come from the stock firmware's wpa_supplicant.conf where there is
# one (TrimUI's own Wi-Fi menu), from networks joined here, and from an
# optional Config/wifi.txt with ssid=... and password=... lines.
# What happens is logged to slot-net.log on the card.
set -u

SD="${SLOT_ROOT:-/mnt/SDCARD}"
SYS="$SD/System"
IFACE="${AGS_NET_IFACE:-wlan0}"
NET="${AGS_NET_SYS:-/sys/class/net}"
RUN="${AGS_RUN:-/tmp}"
WANT="$RUN/slot-wifi.on"
WIFI_TXT="$SD/Config/wifi.txt"
LOG="${AGS_NET_LOG:-$SD/slot-net.log}"
IP="${AGS_IP:-ip}"
WPA_SUPPLICANT="${AGS_WPA_SUPPLICANT:-wpa_supplicant}"
WPA_CLI="${AGS_WPA_CLI:-wpa_cli}"
WPA_PASSPHRASE="${AGS_WPA_PASSPHRASE:-wpa_passphrase}"
UDHCPC="${AGS_UDHCPC:-udhcpc}"

STOCK_CONF="/etc/wifi/wpa_supplicant.conf"
CARD_CONF="$SD/Config/wifi/wpa_supplicant.conf"

log() {
	[ -n "$LOG" ] || return 0
	if [ -f "$LOG" ] && [ "$(wc -c < "$LOG" 2>/dev/null || echo 0)" -gt 200000 ]; then
		mv -f "$LOG" "$LOG.1" 2>/dev/null
	fi
	echo "$(date '+%Y-%m-%d %H:%M:%S') $*" >> "$LOG" 2>/dev/null
	return 0
}

# The stock firmware's own config when it is there and can be written,
# otherwise one kept on the card.
pick_conf() {
	if [ -n "${AGS_NET_CONF:-}" ]; then
		CONF="$AGS_NET_CONF"
	elif [ -f "$STOCK_CONF" ] && [ -w "$STOCK_CONF" ]; then
		CONF="$STOCK_CONF"
	else
		CONF="$CARD_CONF"
	fi
	if [ ! -f "$CONF" ]; then
		mkdir -p "$(dirname "$CONF")" 2>/dev/null
		{
			echo "ctrl_interface=${AGS_NET_CTRL:-$RUN/wpa_supplicant}"
			echo 'update_config=1'
		} > "$CONF" 2>/dev/null || log "cannot write $CONF"
		if [ "$CONF" = "$CARD_CONF" ] && [ -f "$STOCK_CONF" ]; then
			sed -n '/^network={/,/^}/p' "$STOCK_CONF" >> "$CONF" 2>/dev/null
		fi
	fi
	CTRL="${AGS_NET_CTRL:-$(sed -n 's/^ctrl_interface=\(DIR=\)\{0,1\}\([^ ]*\).*/\2/p' "$CONF" 2>/dev/null | head -n 1)}"
	[ -n "$CTRL" ] || CTRL="$RUN/wpa_supplicant"
	mkdir -p "$CTRL" 2>/dev/null
}

cli() {
	$WPA_CLI -p "$CTRL" -i "$IFACE" "$@"
}

value() {
	sed -n "s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*//p" "$WIFI_TXT" 2>/dev/null | head -n 1 | tr -d '\r'
}

# Drops any network block for the SSID in $1 from the config.
forget() {
	tmp="$CONF.tmp"
	awk -v want="ssid=\"$1\"" '
		/^network=\{/ { block = $0 "\n"; inside = 1; hit = 0; next }
		inside { block = block $0 "\n"; if (index($0, want)) hit = 1
			if ($0 ~ /^\}/) { if (!hit) printf "%s", block; inside = 0 }
			next }
		{ print }
	' "$CONF" > "$tmp" 2>/dev/null && mv -f "$tmp" "$CONF"
}

# Adds a network block for SSID $1 with passphrase $2 (empty for open).
remember() {
	forget "$1"
	if [ -z "$2" ]; then
		printf 'network={\n\tssid="%s"\n\tkey_mgmt=NONE\n}\n' "$1" >> "$CONF"
	elif command -v "$WPA_PASSPHRASE" > /dev/null 2>&1 &&
		block="$($WPA_PASSPHRASE "$1" "$2" 2>/dev/null | grep -v '^[[:space:]]*#psk=')" &&
		[ -n "$block" ]; then
		echo "$block" >> "$CONF"
	else
		printf 'network={\n\tssid="%s"\n\tpsk="%s"\n}\n' "$1" "$2" >> "$CONF"
	fi
}

merge_wifi_txt() {
	ssid="$(value ssid)"
	[ -n "$ssid" ] || return 0
	grep -q "ssid=\"$ssid\"" "$CONF" 2>/dev/null && return 0
	remember "$ssid" "$(value password)"
}

wait_iface() {
	w=0
	while [ ! -d "$NET/$IFACE" ] && [ "$w" -lt 40 ]; do
		sleep 0.25 2>/dev/null || sleep 1
		w=$((w + 1))
	done
	[ -d "$NET/$IFACE" ] || { log "$IFACE never appeared"; return 1; }
}

dhcp() {
	if [ -f "$RUN/slot-udhcpc.pid" ] && kill -0 "$(cat "$RUN/slot-udhcpc.pid")" 2>/dev/null; then
		kill -USR1 "$(cat "$RUN/slot-udhcpc.pid")" 2>/dev/null
		return 0
	fi
	# Straight to the log: udhcpc stays behind, holding what it writes to.
	if [ -f "$SYS/udhcpc.sh" ]; then
		$UDHCPC -i "$IFACE" -b -s "$SYS/udhcpc.sh" -p "$RUN/slot-udhcpc.pid" >> "$LOG" 2>&1
	else
		$UDHCPC -i "$IFACE" -b -p "$RUN/slot-udhcpc.pid" >> "$LOG" 2>&1
	fi
}

start() {
	log "up"
	rfkill unblock wifi 2>/dev/null || rfkill.elf unblock wifi 2>/dev/null || true
	wait_iface || return 1
	$IP link set "$IFACE" up 2>/dev/null || true
	pick_conf
	merge_wifi_txt
	log "config $CONF, control $CTRL"
	if ! cli status > /dev/null 2>&1; then
		$WPA_SUPPLICANT -B -i "$IFACE" -c "$CONF" >> "$LOG" 2>&1 ||
			$WPA_SUPPLICANT -B -i "$IFACE" -c "$CONF" -Dnl80211 >> "$LOG" 2>&1 ||
			$WPA_SUPPLICANT -B -i "$IFACE" -c "$CONF" -Dwext >> "$LOG" 2>&1
		s=0
		while ! cli status > /dev/null 2>&1 && [ "$s" -lt 20 ]; do
			sleep 0.25 2>/dev/null || sleep 1
			s=$((s + 1))
		done
		cli status > /dev/null 2>&1 || { log "wpa_supplicant did not start"; return 1; }
	fi
	cli reconfigure > /dev/null 2>&1
	dhcp
	return 0
}

stop() {
	log "down"
	pick_conf
	cli terminate > /dev/null 2>&1
	[ -f "$RUN/slot-udhcpc.pid" ] && kill "$(cat "$RUN/slot-udhcpc.pid")" 2>/dev/null
	rm -f "$RUN/slot-udhcpc.pid"
	$IP addr flush dev "$IFACE" 2>/dev/null
	return 0
}

address() {
	$IP -4 addr show "$IFACE" 2>/dev/null | sed -n 's/.*inet \([0-9.]*\).*/\1/p' | head -n 1
}

scan() {
	start > /dev/null 2>&1 || true
	pick_conf
	cli scan > /dev/null 2>&1
	sleep 4
	# bssid freq signal flags ssid: keep the strongest of each named network.
	cli scan_results 2>/dev/null | tail -n +2 |
		awk -F '\t' 'NF >= 5 && $5 != "" {
			if (!($5 in best) || $3 > best[$5]) { best[$5] = $3; flags[$5] = $4 }
		} END { for (s in best) printf "%s\t%s\t%s\n", best[s], flags[s], s }' |
		sort -t "$(printf '\t')" -k1,1nr
}

join() {
	target="${NET_SSID:-}"
	[ -n "$target" ] || { echo "no NET_SSID" >&2; return 2; }
	touch "$WANT"
	start || true
	pick_conf
	remember "$target" "${NET_PSK:-}"
	log "joining $target"
	cli reconfigure > /dev/null 2>&1
	id="$(cli list_networks 2>/dev/null | awk -F '\t' -v s="$target" '$2 == s { print $1 }' | tail -n 1)"
	[ -n "$id" ] && cli select_network "$id" > /dev/null 2>&1
	dhcp
	return 0
}

case "${1:-}" in
up)
	touch "$WANT"
	start
	;;
down)
	rm -f "$WANT"
	stop
	;;
pause)
	if [ -f "$WANT" ]; then stop; fi
	;;
resume)
	if [ -f "$WANT" ]; then start; fi
	;;
ip)
	a="$(address)"
	[ -n "$a" ] || exit 1
	echo "$a"
	;;
scan)
	scan
	;;
join)
	join
	;;
ssid)
	pick_conf
	cli status 2>/dev/null | sed -n 's/^ssid=//p' | head -n 1
	;;
*)
	echo "usage: net.sh up|down|pause|resume|ip|scan|join|ssid" >&2
	exit 2
	;;
esac
