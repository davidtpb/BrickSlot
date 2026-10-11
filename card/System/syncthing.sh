#!/bin/sh
# Syncthing, so saves and states sync with a computer or another handheld.
#   syncthing.sh start   bring Wi-Fi up and start Syncthing in the background
#   syncthing.sh stop    stop it
# Its settings and keys live in Config/syncthing on the card; its web page is
# http://<device ip>:8384.
set -u

SD="${SLOT_ROOT:-/mnt/SDCARD}"
SYS="$SD/System"
RUN="${AGS_RUN:-/tmp}"
PID="$RUN/slot-syncthing.pid"
HOME_DIR="$SD/Config/syncthing"
BIN="${AGS_SYNCTHING:-$SYS/syncthing}"

running() {
	[ -f "$PID" ] && kill -0 "$(cat "$PID")" 2>/dev/null
}

case "${1:-}" in
start)
	running && exit 0
	[ -x "$BIN" ] || exit 1
	sh "$SYS/net.sh" up
	mkdir -p "$HOME_DIR"
	HOME="$HOME_DIR" STNOUPGRADE=1 STNODEFAULTFOLDER=1 "$BIN" serve --no-browser --no-restart \
		--home="$HOME_DIR" --gui-address=0.0.0.0:8384 \
		> "$HOME_DIR/syncthing.log" 2>&1 &
	echo $! > "$PID"
	;;
stop)
	running && kill "$(cat "$PID")" 2>/dev/null
	rm -f "$PID"
	;;
*)
	echo "usage: syncthing.sh start|stop" >&2
	exit 2
	;;
esac
