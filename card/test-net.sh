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
	printf 'wpa_state=COMPLETED\nssid=Home Net\n'
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
		AGS_NMCLI="${NMCLI_STUB:-$TMP/bin/none}" AGS_MODPROBE="$TMP/bin/none" \
		AGS_WIFI_MODULE_PATH="${MODULE_STUB:-}" sh "$SCRIPT" "$@"
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

[ "$(net status)" = "Radio off" ] || fail "Wi-Fi turned off did not read as off: $(net status)"
mv "$TMP/sys/wlan0" "$TMP/sys/gone"
[ "$(net status)" = "No Wi-Fi chip found" ] || fail "a missing chip was not reported: $(net status)"
mv "$TMP/sys/gone" "$TMP/sys/wlan0"

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
[ "$(net status)" = "Joined Home Net, waiting for an address" ] ||
	fail "a joined network read wrong: $(net status)"

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
[ "$(NMCLI_STUB="$TMP/bin/nmcli" net status)" = "On, not connected" ] ||
	fail "NetworkManager's state read wrong: $(NMCLI_STUB="$TMP/bin/nmcli" net status)"

# An H700 handheld: the driver unloaded, wlan0 absent until it loads, and
# the firmware's own wpa_supplicant running with a config of its own.
H="$TMP/h700"
mkdir -p "$H/sys" "$H/proc/4242" "$H/card/Config" "$H/run"
: > "$H/8821cs.ko"
printf 'ctrl_interface=/var/run/wpa_supplicant\nnetwork={\n\tssid="Stock Net"\n\tpsk="stockpass"\n}\n' > "$H/stock.conf"
printf 'wpa_supplicant\0-B\0-i\0wlan0\0-c\0%s\0' "$H/stock.conf" > "$H/proc/4242/cmdline"
printf '#!/bin/sh\necho "insmod $*" >> "%s"\nmkdir -p "%s/sys/wlan0"\n' "$LOG" "$H" > "$TMP/bin/insmod"
printf '#!/bin/sh\necho "pgrep $*" >> "%s"\n[ -f "%s/stock.gone" ] || echo 4242\n' "$LOG" "$H" > "$TMP/bin/pgrep"
printf '#!/bin/sh\nexit 0\n' > "$TMP/bin/lsmod"
chmod +x "$TMP/bin/insmod" "$TMP/bin/pgrep" "$TMP/bin/lsmod"
rm -f "$UP"
: > "$LOG"
h700() {
	SLOT_ROOT="$H/card" AGS_RUN="$H/run" AGS_IP="$TMP/bin/ip" AGS_NET_SYS="$H/sys" \
		AGS_WPA_SUPPLICANT="$TMP/bin/wpa_supplicant" AGS_WPA_CLI="$TMP/bin/wpa_cli" \
		AGS_WPA_PASSPHRASE="$TMP/bin/none" AGS_UDHCPC="$TMP/bin/udhcpc" \
		AGS_NMCLI="$TMP/bin/none" AGS_WIFI_MODULE_PATH="$H/8821cs.ko" \
		AGS_INSMOD="$TMP/bin/insmod" AGS_LSMOD="$TMP/bin/lsmod" AGS_PGREP="$TMP/bin/pgrep" \
		AGS_PROC="$H/proc" AGS_RAIL_SETTLE=0 sh "$SCRIPT" "$@"
}
h700 up
grep -q "^insmod $H/8821cs.ko" "$LOG" || fail "the H700 Wi-Fi driver was not loaded"
grep -q '^wpa_supplicant -B -D nl80211 -i wlan0 -c' "$LOG" || fail "the H700 wpa_supplicant was not started with nl80211"
C="$H/card/Config/wifi/wpa_supplicant.conf"
grep -q '^ctrl_interface=/var/run/wpa_supplicant$' "$C" || fail "the H700 control socket is not where link play looks"
grep -q 'ssid="Stock Net"' "$C" || fail "the firmware's networks were not adopted"
h700 up
[ "$(grep -c 'ssid="Stock Net"' "$C")" = 1 ] || fail "the firmware's networks were adopted twice"
grep -q 'taking over from wpa_supplicant 4242' "$H/card/slot-net.log" || fail "the firmware's wpa_supplicant was left running"

echo "net: all passed"
