#!/bin/sh
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRIPT="$HERE/System/net.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

mkdir -p "$TMP/card/Config" "$TMP/run" "$TMP/bin"
LOG="$TMP/calls"
for tool in ip wpa_supplicant wpa_cli udhcpc; do
	printf '#!/bin/sh\necho "%s $*" >> "%s"\n' "$tool" "$LOG" > "$TMP/bin/$tool"
	chmod +x "$TMP/bin/$tool"
done

net() {
	SLOT_ROOT="$TMP/card" AGS_RUN="$TMP/run" AGS_NET_CONF="$TMP/etc/wpa.conf" \
		AGS_NET_CTRL="$TMP/etc/sockets" AGS_IP="$TMP/bin/ip" \
		AGS_WPA_SUPPLICANT="$TMP/bin/wpa_supplicant" AGS_WPA_CLI="$TMP/bin/wpa_cli" \
		AGS_UDHCPC="$TMP/bin/udhcpc" sh "$SCRIPT" "$@"
}

fail() {
	echo "net: $1" >&2
	exit 1
}

printf 'ssid = Home Net\r\npassword=hunter22\n' > "$TMP/card/Config/wifi.txt"
net up
grep -q 'ssid="Home Net"' "$TMP/etc/wpa.conf" || fail "wifi.txt network was not added"
grep -q 'psk="hunter22"' "$TMP/etc/wpa.conf" || fail "wifi.txt password was not added"
grep -q "ctrl_interface=$TMP/etc/sockets" "$TMP/etc/wpa.conf" || fail "no control socket"
[ -f "$TMP/run/slot-wifi.on" ] || fail "up did not remember Wi-Fi is on"
grep -q '^udhcpc -i wlan0' "$LOG" || fail "no dhcp client was started"

net up
[ "$(grep -c 'ssid="Home Net"' "$TMP/etc/wpa.conf")" = 1 ] || fail "network added twice"

net pause
[ -f "$TMP/run/slot-wifi.on" ] || fail "pause forgot Wi-Fi was on"
grep -q 'terminate' "$LOG" || fail "pause did not let go of wlan0"

: > "$LOG"
net resume
grep -q '^wpa_cli .* reconfigure' "$LOG" || fail "resume did not bring Wi-Fi back"

net down
[ ! -f "$TMP/run/slot-wifi.on" ] || fail "down left Wi-Fi marked on"
: > "$LOG"
net resume
[ ! -s "$LOG" ] || fail "resume brought Wi-Fi back after it was turned off"

echo "net: all passed"
