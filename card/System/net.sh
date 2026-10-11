#!/bin/sh
# Wi-Fi as a client of the user's own network.
#   net.sh up      turn Wi-Fi on and keep it on across link play
#   net.sh down    turn it off
#   net.sh pause   let go of wlan0 for link play, remembering it was on
#   net.sh resume  take wlan0 back if it was on before the pause
#   net.sh ip      print wlan0's address, if it has one
# Networks come from the stock firmware's wpa_supplicant.conf (set up from
# TrimUI's own Wi-Fi menu) plus an optional Config/wifi.txt with
# ssid=... and password=... lines.
set -u

SD="${SLOT_ROOT:-/mnt/SDCARD}"
IFACE="${AGS_NET_IFACE:-wlan0}"
CONF="${AGS_NET_CONF:-/etc/wifi/wpa_supplicant.conf}"
CTRL="${AGS_NET_CTRL:-/etc/wifi/sockets}"
RUN="${AGS_RUN:-/tmp}"
WANT="$RUN/slot-wifi.on"
WIFI_TXT="$SD/Config/wifi.txt"
IP="${AGS_IP:-ip}"
WPA_SUPPLICANT="${AGS_WPA_SUPPLICANT:-wpa_supplicant}"
WPA_CLI="${AGS_WPA_CLI:-wpa_cli}"
UDHCPC="${AGS_UDHCPC:-udhcpc}"

value() {
	sed -n "s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*//p" "$WIFI_TXT" 2>/dev/null | head -n 1 | tr -d '\r'
}

conf_base() {
	mkdir -p "$(dirname "$CONF")" "$CTRL" 2>/dev/null
	[ -f "$CONF" ] && return 0
	{
		echo "ctrl_interface=$CTRL"
		echo 'update_config=1'
	} > "$CONF"
}

merge_wifi_txt() {
	ssid="$(value ssid)"
	[ -n "$ssid" ] || return 0
	grep -q "ssid=\"$ssid\"" "$CONF" 2>/dev/null && return 0
	password="$(value password)"
	{
		echo 'network={'
		echo "	ssid=\"$ssid\""
		if [ -n "$password" ]; then
			echo "	psk=\"$password\""
		else
			echo '	key_mgmt=NONE'
		fi
		echo '}'
	} >> "$CONF"
}

start() {
	rfkill unblock wifi 2>/dev/null || rfkill.elf unblock wifi 2>/dev/null || true
	$IP link set "$IFACE" up 2>/dev/null || true
	conf_base
	merge_wifi_txt
	if ! $WPA_CLI -p "$CTRL" -i "$IFACE" status > /dev/null 2>&1; then
		$WPA_SUPPLICANT -B -i "$IFACE" -c "$CONF" > /dev/null 2>&1 ||
			$WPA_SUPPLICANT -B -i "$IFACE" -c "$CONF" -Dnl80211 > /dev/null 2>&1
	fi
	$WPA_CLI -p "$CTRL" -i "$IFACE" reconfigure > /dev/null 2>&1
	if [ ! -f "$RUN/slot-udhcpc.pid" ] || ! kill -0 "$(cat "$RUN/slot-udhcpc.pid")" 2>/dev/null; then
		$UDHCPC -i "$IFACE" -b -p "$RUN/slot-udhcpc.pid" > /dev/null 2>&1
	fi
	return 0
}

stop() {
	$WPA_CLI -p "$CTRL" -i "$IFACE" terminate > /dev/null 2>&1
	[ -f "$RUN/slot-udhcpc.pid" ] && kill "$(cat "$RUN/slot-udhcpc.pid")" 2>/dev/null
	rm -f "$RUN/slot-udhcpc.pid"
	$IP addr flush dev "$IFACE" 2>/dev/null
	return 0
}

address() {
	$IP -4 addr show "$IFACE" 2>/dev/null | sed -n 's/.*inet \([0-9.]*\).*/\1/p' | head -n 1
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
*)
	echo "usage: net.sh up|down|pause|resume|ip" >&2
	exit 2
	;;
esac
