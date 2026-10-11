#!/bin/sh
# Bluetooth on TrimUI's stock firmware and on the H700 handhelds alike.
#   bt.sh up              power the radio on, with the daemons it needs
#   bt.sh down            power it off
#   bt.sh scan            look for devices for a while, then list them
#   bt.sh list            list the devices known, one per line:
#                         address<TAB>flags<TAB>name
#                         flags: p paired, c connected, a plays audio
#   bt.sh connect         pair with, trust and connect BT_MAC
#   bt.sh disconnect      disconnect BT_MAC
#   bt.sh forget          unpair BT_MAC
#   bt.sh status          print in a few words what Bluetooth is doing
# Headphones play through BlueALSA where the firmware has it; slot opens
# its bluealsa device once a device that plays audio is connected.
# What happens is logged to slot-bt.log on the card.
set -u

SD="${SLOT_ROOT:-/mnt/SDCARD}"
SYS="$SD/System"
LOG="${AGS_BT_LOG:-$SD/slot-bt.log}"
CTL="${AGS_BLUETOOTHCTL:-bluetoothctl}"
PGREP="${AGS_PGREP:-pgrep}"
HCI="${AGS_HCI_SYS:-/sys/class/bluetooth}"
RFKILL_SYS="${AGS_RFKILL_SYS:-/sys/class/rfkill}"
SCAN_S="${AGS_BT_SCAN_S:-10}"
PAIR_S="${AGS_BT_PAIR_S:-8}"
SETTLE="${AGS_BT_SETTLE:-1}"
MAC="${BT_MAC:-}"

PATH="$SYS/bin:$PATH:/usr/libexec/bluetooth:/usr/lib/bluetooth:/usr/sbin:/sbin"
export PATH

log() {
	[ -n "$LOG" ] || return 0
	echo "$(date '+%F %T' 2>/dev/null) $*" >> "$LOG" 2>/dev/null
}

have() {
	command -v "$1" >/dev/null 2>&1
}

running() {
	$PGREP -x "$1" >/dev/null 2>&1 || $PGREP -f "$1" >/dev/null 2>&1
}

hci_up() {
	[ -e "$HCI/hci0" ]
}

wait_hci() {
	i=0
	while [ "$i" -lt "${1:-8}" ]; do
		hci_up && return 0
		sleep 1
		i=$((i + 1))
	done
	hci_up
}

# Some bluetoothctl builds leave colour codes and prompts on every line.
plain() {
	sed 's/\x1b\[[0-9;]*m//g; s/\r//g; s/^\[[^]]*\][#>] *//'
}

ctl() {
	$CTL "$@" </dev/null 2>>"$LOG" | plain
}

unblock() {
	have rfkill && rfkill unblock bluetooth >>"$LOG" 2>&1
	for r in "$RFKILL_SYS"/rfkill*; do
		[ -f "$r/type" ] || continue
		[ "$(cat "$r/type" 2>/dev/null)" = bluetooth ] || continue
		echo 1 > "$r/state" 2>/dev/null
	done
}

# Brings up the controller: the firmware's own script where there is one
# (TrimUI), else the Realtek UART attach the H700 boards need.
attach() {
	hci_up && return 0
	for s in /etc/bluetooth/bt_init.sh /etc/init.d/bluetooth /etc/init.d/S40bluetooth; do
		[ -x "$s" ] || continue
		log "attach: $s start"
		"$s" start >>"$LOG" 2>&1
		wait_hci 6 && return 0
	done
	if have rtk_hciattach; then
		for tty in /dev/ttyS1 /dev/ttyS2; do
			[ -e "$tty" ] || continue
			log "attach: rtk_hciattach on $tty"
			rtk_hciattach -n -s 115200 "$tty" rtk_h5 >>"$LOG" 2>&1 &
			wait_hci 8 && return 0
			kill $! 2>/dev/null
		done
	fi
	if have hciattach; then
		log "attach: hciattach"
		hciattach -s 115200 /dev/ttyS1 any >>"$LOG" 2>&1
		wait_hci 6 && return 0
	fi
	log "attach: no controller came up"
	return 1
}

daemons() {
	if ! running dbus-daemon && have dbus-daemon; then
		log "starting dbus"
		mkdir -p /var/run/dbus 2>/dev/null
		dbus-daemon --system --fork >>"$LOG" 2>&1
	fi
	if ! running bluetoothd; then
		if have bluetoothd; then
			log "starting bluetoothd"
			bluetoothd -n >>"$LOG" 2>&1 &
			sleep "$SETTLE"
		else
			log "no bluetoothd on this system"
		fi
	fi
	have hciconfig && hciconfig hci0 up >>"$LOG" 2>&1
	if ! running bluealsa; then
		for b in bluealsad bluealsa; do
			have "$b" || continue
			log "starting $b"
			"$b" -p a2dp-source >>"$LOG" 2>&1 &
			sleep "$SETTLE"
			break
		done
	fi
}

up() {
	log "up"
	unblock
	attach
	daemons
	ctl power on >>"$LOG"
	ctl pairable on >>"$LOG"
	status >>"$LOG"
}

down() {
	log "down"
	ctl power off >>"$LOG"
}

# address<TAB>flags<TAB>name for every device bluetoothctl knows with a name.
list() {
	ctl devices | while read -r kind addr name; do
		[ "$kind" = Device ] || continue
		[ -n "$addr" ] || continue
		dashed="$(echo "$addr" | tr : -)"
		[ -n "$name" ] && [ "$name" != "$dashed" ] || continue
		flags=""
		info="$(ctl info "$addr")"
		echo "$info" | grep -q "Paired: yes" && flags="${flags}p"
		echo "$info" | grep -q "Connected: yes" && flags="${flags}c"
		echo "$info" | grep -qi "Audio Sink\|Headset\|Handsfree\|0000110b" && flags="${flags}a"
		printf '%s\t%s\t%s\n' "$addr" "${flags:--}" "$name"
	done
}

# Talks to one bluetoothctl for a while, so its agent is there to answer
# the device while pairing.
session() {
	secs="$1"
	shift
	{
		echo "agent NoInputNoOutput"
		echo "default-agent"
		for c in "$@"; do
			echo "$c"
			sleep 1
		done
		sleep "$secs"
		echo "quit"
	} | $CTL >>"$LOG" 2>&1
}

scan() {
	log "scan for ${SCAN_S}s"
	hci_up || up
	session "$SCAN_S" "scan on"
	ctl scan off >/dev/null
	found="$(list)"
	log "scan found $(echo "$found" | grep -c .) devices"
	echo "$found"
}

connect() {
	[ -n "$MAC" ] || { echo "no BT_MAC" >&2; exit 2; }
	log "connect $MAC"
	if ! ctl info "$MAC" | grep -q "Paired: yes"; then
		session "$PAIR_S" "pair $MAC"
	fi
	ctl trust "$MAC" >>"$LOG"
	ctl connect "$MAC" >>"$LOG"
	i=0
	while [ "$i" -lt 8 ]; do
		if ctl info "$MAC" | grep -q "Connected: yes"; then
			log "connected $MAC"
			list | grep "^$MAC	"
			return 0
		fi
		sleep 1
		i=$((i + 1))
	done
	log "connect $MAC failed"
	return 1
}

status() {
	if ! hci_up; then
		echo "No Bluetooth controller"
		return
	fi
	if ! ctl show | grep -q "Powered: yes"; then
		echo "Off"
		return
	fi
	on="$(list | awk -F '\t' 'index($2, "c") { print $3 }' | head -n 1)"
	if [ -n "$on" ]; then
		echo "Connected to $on"
	elif running bluealsa; then
		echo "On, nothing connected"
	else
		echo "On, no BlueALSA for audio"
	fi
}

case "${1:-}" in
up) up ;;
down) down ;;
scan) scan ;;
list) list ;;
connect) connect ;;
disconnect)
	log "disconnect $MAC"
	ctl disconnect "$MAC" >>"$LOG"
	;;
forget)
	log "forget $MAC"
	ctl remove "$MAC" >>"$LOG"
	;;
status) status ;;
*)
	echo "usage: bt.sh up|down|scan|list|connect|disconnect|forget|status" >&2
	exit 2
	;;
esac
