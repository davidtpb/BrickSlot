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
NMCLI="${AGS_NMCLI:-nmcli}"
MODPROBE="${AGS_MODPROBE:-modprobe}"

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

# Finds a wpa_supplicant already looking after the interface, the system's
# own or one started earlier, and talks to it from then on.
find_running() {
	for d in "$CTRL" /var/run/wpa_supplicant /run/wpa_supplicant /etc/wifi/sockets \
		/tmp/wpa_supplicant "$RUN/wpa_supplicant"; do
		[ -n "$d" ] || continue
		if $WPA_CLI -p "$d" -i "$IFACE" status > /dev/null 2>&1; then
			CTRL="$d"
			return 0
		fi
	done
	return 1
}

# Writes what this system has to work with to the log, for when Wi-Fi
# will not come up.
diagnose() {
	log "tools: $(for t in "$WPA_SUPPLICANT" "$WPA_CLI" "$UDHCPC" "$NMCLI" connmanctl iw iwlist rfkill ip; do
		command -v "$t" > /dev/null 2>&1 && printf '%s ' "$t" || printf '(no %s) ' "$t"
	done)"
	log "interfaces: $(ls "$NET" 2>/dev/null | tr '\n' ' ')"
	log "rfkill: $(rfkill list 2>/dev/null | tr '\n' ' ' | head -c 300)"
	log "running: $(ps 2>/dev/null | grep -E 'wpa_supplicant|udhcpc|hostapd|connman|NetworkManager' | grep -v grep | tr -s ' ' | tr '\n' ';' | head -c 400)"
	log "control: $CTRL status: $(cli status 2>&1 | tr '\n' ' ' | head -c 300)"
	log "modules: $(lsmod 2>/dev/null | awk 'NR > 1 { printf "%s ", $1 }' | head -c 400)"
	log "kernel: $(uname -a 2>/dev/null)"
	log "os: $(head -n 3 /etc/os-release 2>/dev/null | tr '\n' ' ')"
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

# Whether NetworkManager looks after Wi-Fi here, as on some H700 systems.
# A wpa_supplicant of our own beside it would find nothing.
nm() {
	command -v "$NMCLI" > /dev/null 2>&1 &&
		$NMCLI -t -f RUNNING general 2>/dev/null | grep -q '^running'
}

# Loads the Wi-Fi driver the H700 handhelds use when the interface has
# not appeared on its own.
load_driver() {
	[ -d "$NET/$IFACE" ] && return 0
	for m in 8821cs rtl8821cs 8723ds 8189fs aic8800_fdrv; do
		$MODPROBE "$m" > /dev/null 2>&1 && log "loaded $m" && return 0
	done
	return 1
}

nm_scan() {
	$NMCLI radio wifi on > /dev/null 2>&1
	# SIGNAL is a percentage; turned into dBm here to match wpa_supplicant.
	# Colons inside a field come escaped as \:.
	$NMCLI -t -f SIGNAL,SECURITY,SSID dev wifi list --rescan yes 2>/dev/null |
		awk '{
			line = $0; gsub(/\\:/, "\001", line)
			n = split(line, f, ":")
			ssid = f[3]; for (i = 4; i <= n; i++) ssid = ssid ":" f[i]
			gsub(/\001/, ":", ssid)
			sec = (f[2] == "" || f[2] == "--") ? "[ESS]" : "[WPA2-PSK]"
			printf "%d\t%s\t%s\n", f[1] / 2 - 100, sec, ssid
		}'
}

nm_join() {
	log "joining $1 through NetworkManager"
	$NMCLI radio wifi on > /dev/null 2>&1
	if [ -n "$2" ]; then
		$NMCLI dev wifi connect "$1" password "$2" ifname "$IFACE" >> "$LOG" 2>&1
	else
		$NMCLI dev wifi connect "$1" ifname "$IFACE" >> "$LOG" 2>&1
	fi
}

wait_iface() {
	load_driver
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
	if nm; then
		log "NetworkManager looks after Wi-Fi here"
		$NMCLI dev set "$IFACE" managed yes > /dev/null 2>&1
		$NMCLI radio wifi on >> "$LOG" 2>&1
		return 0
	fi
	wait_iface || return 1
	$IP link set "$IFACE" up 2>/dev/null || true
	pick_conf
	merge_wifi_txt
	if find_running; then
		log "using the wpa_supplicant already at $CTRL"
		dhcp
		return 0
	fi
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
	if nm; then
		# Let go of the interface for link play rather than turn the radio off.
		$NMCLI dev disconnect "$IFACE" > /dev/null 2>&1
		$NMCLI dev set "$IFACE" managed no > /dev/null 2>&1
		return 0
	fi
	pick_conf
	find_running || true
	cli terminate > /dev/null 2>&1
	[ -f "$RUN/slot-udhcpc.pid" ] && kill "$(cat "$RUN/slot-udhcpc.pid")" 2>/dev/null
	rm -f "$RUN/slot-udhcpc.pid"
	$IP addr flush dev "$IFACE" 2>/dev/null
	return 0
}

address() {
	$IP -4 addr show "$IFACE" 2>/dev/null | sed -n 's/.*inet \([0-9.]*\).*/\1/p' | head -n 1
}

# Keeps the strongest of each named network from signal<TAB>flags<TAB>ssid
# lines, strongest first.
best() {
	awk -F '\t' 'NF >= 3 && $3 != "" {
		if (!($3 in best) || $1 > best[$3]) { best[$3] = $1; flags[$3] = $2 }
	} END { for (s in best) printf "%s\t%s\t%s\n", best[s], flags[s], s }' |
		sort -t "$(printf '\t')" -k1,1nr
}

# The same lines from iw's scan, for when wpa_supplicant gives none.
iw_scan() {
	iw dev "$IFACE" scan 2>/dev/null | awk '
		function out() { if (ssid != "") printf "%d\t%s\t%s\n", sig, flags, ssid }
		/^BSS / { out(); ssid = ""; sig = -100; flags = "[ESS]" }
		/signal:/ { sig = int($2) }
		/SSID: / { sub(/^[ \t]*SSID: /, ""); ssid = $0 }
		/RSN:|WPA:/ { flags = "[WPA2-PSK]" }
		/Privacy/ { if (flags == "[ESS]") flags = "[WEP]" }
		END { out() }'
}

# And from iwlist's, the oldest of the three.
iwlist_scan() {
	iwlist "$IFACE" scan 2>/dev/null | awk '
		function out() { if (ssid != "") printf "%d\t%s\t%s\n", sig, flags, ssid }
		/Cell [0-9]+/ { out(); ssid = ""; sig = -100; flags = "[ESS]" }
		/Signal level=/ { s = $0; sub(/.*Signal level=/, "", s); sig = int(s) }
		/ESSID:/ { s = $0; sub(/.*ESSID:"/, "", s); sub(/"$/, "", s); ssid = s }
		/Encryption key:on/ { flags = "[WPA2-PSK]" }
		END { out() }'
}

scan() {
	start > /dev/null 2>&1 || true
	if nm; then
		found="$(nm_scan | best)"
		[ -n "$found" ] || { log "NetworkManager found no networks"; diagnose; }
		[ -n "$found" ] && echo "$found"
		return 0
	fi
	pick_conf
	find_running || true
	cli scan > /dev/null 2>&1
	sleep 4
	# bssid freq signal flags ssid
	found="$(cli scan_results 2>/dev/null | tail -n +2 |
		awk -F '\t' 'NF >= 5 { printf "%s\t%s\t%s\n", $3, $4, $5 }' | best)"
	if [ -z "$found" ] && command -v iw > /dev/null 2>&1; then
		log "wpa_supplicant found nothing, asking iw"
		found="$(iw_scan | best)"
	fi
	if [ -z "$found" ] && command -v iwlist > /dev/null 2>&1; then
		log "asking iwlist"
		found="$(iwlist_scan | best)"
	fi
	if [ -z "$found" ]; then
		log "scan found no networks"
		diagnose
	else
		log "scan found $(echo "$found" | wc -l) networks"
	fi
	[ -n "$found" ] && echo "$found"
	return 0
}

join() {
	target="${NET_SSID:-}"
	[ -n "$target" ] || { echo "no NET_SSID" >&2; return 2; }
	touch "$WANT"
	start || true
	if nm; then
		nm_join "$target" "${NET_PSK:-}"
		return 0
	fi
	pick_conf
	find_running || true
	remember "$target" "${NET_PSK:-}"
	log "joining $target through $CTRL"
	# Told to the running wpa_supplicant directly, so it does not matter
	# whose config it read when it started.
	for old in $(cli list_networks 2>/dev/null | awk -F '\t' -v s="$target" 'NR > 1 && $2 == s { print $1 }'); do
		cli remove_network "$old" > /dev/null 2>&1
	done
	id="$(cli add_network 2>/dev/null | tail -n 1 | tr -dc '0-9')"
	if [ -z "$id" ]; then
		log "wpa_supplicant would not take a new network"
		diagnose
		return 1
	fi
	cli set_network "$id" ssid "\"$target\"" > /dev/null 2>&1
	if [ -n "${NET_PSK:-}" ]; then
		cli set_network "$id" psk "\"$NET_PSK\"" > /dev/null 2>&1
	else
		cli set_network "$id" key_mgmt NONE > /dev/null 2>&1
	fi
	cli enable_network "$id" > /dev/null 2>&1
	cli select_network "$id" > /dev/null 2>&1
	cli save_config > /dev/null 2>&1
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
