#!/bin/sh
set -eu
[ -n "${NET_TEST_TRACE:-}" ] && set -x

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRIPT="$HERE/System/net.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

mkdir -p "$TMP/card/Config" "$TMP/run" "$TMP/bin" "$TMP/sys/wlan0"
LOG="$TMP/calls"
for tool in ip udhcpc; do
	printf '#!/bin/sh\necho "%s $*" >> "%s"\n' "$tool" "$LOG" > "$TMP/bin/$tool"
	chmod +x "$TMP/bin/$tool"
done
UP="$TMP/run/wpa.up"
printf '#!/bin/sh\necho "wpa_supplicant $*" >> "%s"\ntouch "%s"\n' "$LOG" "$UP" > "$TMP/bin/wpa_supplicant"
chmod +x "$TMP/bin/wpa_supplicant"
cat > "$TMP/bin/wpa_cli" <<STUB
#!/bin/sh
echo "wpa_cli \$*" >> "$LOG"
case "\$*" in
*status*)
	[ -f "$UP" ] || exit 1
	;;
*terminate*)
	rm -f "$UP"
	;;
*scan_results*)
	printf 'bssid / frequency / signal level / flags / ssid\n'
	printf 'aa:01\t2412\t-70\t[WPA2-PSK-CCMP][ESS]\tHome Net\n'
	printf 'aa:02\t5180\t-48\t[WPA2-PSK-CCMP][ESS]\tHome Net\n'
	printf 'aa:03\t2437\t-60\t[ESS]\tCafe\n'
	printf 'aa:04\t2462\t-80\t[WPA2-PSK-CCMP][ESS]\t\n'
	;;
*list_networks*)
	printf 'network id / ssid / bssid / flags\n'
	printf '0\tHome Net\tany\t\n1\tNew Place\tany\t\n'
	;;
*add_network*)
	echo 2
	;;
esac
exit 0
STUB
chmod +x "$TMP/bin/wpa_cli"

net() {
	SLOT_ROOT="$TMP/card" AGS_RUN="$TMP/run" AGS_NET_CONF="$TMP/etc/wpa.conf" \
		AGS_NET_CTRL="$TMP/etc/sockets" AGS_IP="$TMP/bin/ip" AGS_NET_SYS="$TMP/sys" \
		AGS_WPA_SUPPLICANT="$TMP/bin/wpa_supplicant" AGS_WPA_CLI="$TMP/bin/wpa_cli" \
		AGS_WPA_PASSPHRASE="$TMP/bin/none" AGS_UDHCPC="$TMP/bin/udhcpc" \
		AGS_NMCLI="${NMCLI_STUB:-$TMP/bin/none}" AGS_MODPROBE="$TMP/bin/none" sh "$SCRIPT" "$@"
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
[ -s "$TMP/card/slot-net.log" ] || fail "nothing was logged"

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

out="$(net scan)"
first="$(echo "$out" | head -n 1)"
[ "$(echo "$first" | cut -f3)" = "Home Net" ] || fail "the strongest network is not first: $out"
[ "$(echo "$first" | cut -f1)" = "-48" ] || fail "a network kept its weaker signal: $out"
[ "$(echo "$out" | grep -c 'Home Net')" = 1 ] || fail "a network was listed twice: $out"
echo "$out" | grep -q "$(printf '\tCafe$')" || fail "an open network is missing: $out"
[ "$(echo "$out" | wc -l)" = 2 ] || fail "a hidden network was listed: $out"

: > "$LOG"
NET_SSID="New Place" NET_PSK='p@ss "word"' net join
grep -q 'ssid="New Place"' "$TMP/etc/wpa.conf" || fail "join did not remember the network"
grep -q 'remove_network 1' "$LOG" || fail "join kept the old entry for the network"
grep -q "set_network 2 ssid \"New Place\"" "$LOG" || fail "join did not name the network"
grep -q 'set_network 2 psk "p@ss "word""' "$LOG" || fail "join did not give the password"
grep -q 'select_network 2' "$LOG" || fail "join did not switch to the network"
[ -f "$TMP/run/slot-wifi.on" ] || fail "join did not leave Wi-Fi on"
NET_SSID="New Place" NET_PSK='other' net join
[ "$(grep -c 'ssid="New Place"' "$TMP/etc/wpa.conf")" = 1 ] || fail "joining again kept the old password too"
grep -q 'psk="other"' "$TMP/etc/wpa.conf" || fail "the new password was not kept"
grep -q 'ssid="Home Net"' "$TMP/etc/wpa.conf" || fail "joining forgot another network"

NET_SSID="Cafe" NET_PSK="" net join
awk '/ssid="Cafe"/{f=1} f&&/key_mgmt=NONE/{ok=1} END{exit !ok}' "$TMP/etc/wpa.conf" ||
	fail "an open network was not remembered without a password"
grep -q 'set_network 2 key_mgmt NONE' "$LOG" || fail "an open network was joined with a password"

net down
touch "$UP"
: > "$LOG"
net up
grep -q '^wpa_supplicant' "$LOG" && fail "a second wpa_supplicant was started beside the running one"
grep -q '^udhcpc -i wlan0' "$LOG" || fail "no address was asked for through the running one"
grep -q 'using the wpa_supplicant already' "$TMP/card/slot-net.log" || fail "the running one was not noted"

# A system where NetworkManager looks after Wi-Fi.
cat > "$TMP/bin/nmcli" <<STUB
#!/bin/sh
echo "nmcli \$*" >> "$LOG"
case "\$*" in
*RUNNING*) echo running ;;
*"wifi list"*)
	printf '90:WPA2:Home Net\n40:WPA2:Home Net\n60::Cafe\n70:WPA1 WPA2:Colon\\\\:Net\n'
	;;
esac
exit 0
STUB
chmod +x "$TMP/bin/nmcli"
: > "$LOG"
out="$(NMCLI_STUB="$TMP/bin/nmcli" net scan)"
[ "$(echo "$out" | head -n 1)" = "$(printf -- '-55\t[WPA2-PSK]\tHome Net')" ] || fail "NetworkManager's networks read wrong: $out"
echo "$out" | grep -q "$(printf '\t\[ESS\]\tCafe$')" || fail "an open network was taken as locked: $out"
echo "$out" | grep -q "$(printf '\tColon:Net$')" || fail "a colon in a name was lost: $out"
grep -q '^wpa_supplicant' "$LOG" && fail "a wpa_supplicant was started beside NetworkManager"
NMCLI_STUB="$TMP/bin/nmcli" NET_SSID="Home Net" NET_PSK="hunter22" net join
grep -q 'nmcli dev wifi connect Home Net password hunter22 ifname wlan0' "$LOG" || fail "NetworkManager was not asked to join"
NMCLI_STUB="$TMP/bin/nmcli" net pause
grep -q 'nmcli dev set wlan0 managed no' "$LOG" || fail "link play would fight NetworkManager for wlan0"

echo "net: all passed"
