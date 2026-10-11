#!/bin/sh
# Runs bt.sh against a stand-in bluetoothctl and checks what it prints.
set -eu
[ -n "${BT_TEST_TRACE:-}" ] && set -x

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRIPT="$HERE/System/bt.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

mkdir -p "$TMP/card" "$TMP/bin" "$TMP/hci/hci0" "$TMP/state"
CALLS="$TMP/calls"
cat > "$TMP/bin/bluetoothctl" <<STUB
#!/bin/sh
echo "bluetoothctl \$*" >> "$CALLS"
if [ \$# -eq 0 ]; then
	while read -r line; do
		echo "session \$line" >> "$CALLS"
		case "\$line" in
		pair\ *) touch "$TMP/state/paired" ;;
		esac
	done
	exit 0
fi
case "\$1" in
devices)
	printf '\033[0;94m[bluetooth]\033[0m# Device AA:BB:CC:DD:EE:01 Buds Pro\n'
	printf 'Device AA:BB:CC:DD:EE:02 AA-BB-CC-DD-EE-02\n'
	printf 'Device AA:BB:CC:DD:EE:03 8BitDo Pad\n'
	;;
info)
	case "\$2" in
	AA:BB:CC:DD:EE:01)
		[ -f "$TMP/state/paired" ] && echo "	Paired: yes" || echo "	Paired: no"
		[ -f "$TMP/state/connected" ] && echo "	Connected: yes" || echo "	Connected: no"
		echo "	UUID: Audio Sink                (0000110b-0000-1000-8000-00805f9b34fb)"
		;;
	*)
		echo "	Paired: no"
		echo "	Connected: no"
		;;
	esac
	;;
connect)
	[ -f "$TMP/state/paired" ] && touch "$TMP/state/connected"
	;;
show)
	echo "	Powered: yes"
	;;
esac
exit 0
STUB
chmod +x "$TMP/bin/bluetoothctl"
printf '#!/bin/sh\nexit 1\n' > "$TMP/bin/pgrep"
chmod +x "$TMP/bin/pgrep"

bt() {
	SLOT_ROOT="$TMP/card" AGS_BT_LOG="$TMP/log" AGS_HCI_SYS="$TMP/hci" \
		AGS_RFKILL_SYS="$TMP/none" AGS_BT_SCAN_S=0 AGS_BT_PAIR_S=0 AGS_BT_SETTLE=0 \
		AGS_PGREP="$TMP/bin/pgrep" PATH="$TMP/bin:/usr/bin:/bin" \
		sh "$SCRIPT" "$@"
}

fail() {
	echo "FAIL: $*" >&2
	exit 1
}

out="$(bt list)"
[ "$(echo "$out" | wc -l)" -eq 2 ] || fail "list kept a nameless device: $out"
echo "$out" | grep -q "^AA:BB:CC:DD:EE:01	a	Buds Pro$" || fail "list: $out"
echo "$out" | grep -q "^AA:BB:CC:DD:EE:03	-	8BitDo Pad$" || fail "list: $out"

out="$(bt scan)"
grep -q "session scan on" "$CALLS" || fail "scan never asked to scan"
echo "$out" | grep -q "Buds Pro" || fail "scan listed: $out"

out="$(BT_MAC=AA:BB:CC:DD:EE:01 bt connect)" || fail "connect failed"
grep -q "session pair AA:BB:CC:DD:EE:01" "$CALLS" || fail "connect did not pair"
grep -q "session agent NoInputNoOutput" "$CALLS" || fail "no agent while pairing"
grep -q "bluetoothctl trust AA:BB:CC:DD:EE:01" "$CALLS" || fail "not trusted"
[ "$out" = "AA:BB:CC:DD:EE:01	pca	Buds Pro" ] || fail "connect printed: $out"

: > "$CALLS"
BT_MAC=AA:BB:CC:DD:EE:01 bt connect >/dev/null || fail "reconnect failed"
grep -q "session pair" "$CALLS" && fail "paired again"

[ "$(bt status)" = "Connected to Buds Pro" ] || fail "status: $(bt status)"
rm -rf "$TMP/hci/hci0"
[ "$(bt status)" = "No Bluetooth controller" ] || fail "status without hci"

echo "bt.sh: all good"
