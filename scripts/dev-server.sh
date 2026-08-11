#!/usr/bin/env bash
# Dev helper: run the Vautr server persistently with a heartbeat so it can be
# managed as a background task, auto-restarting if it crashes. Only for local dev.
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="${VAUTR_SERVER_BIN:-target/debug/vautr-server}"
echo "Starting Vautr server: $BIN"
while true; do
  "$BIN" &
  SRV_PID=$!
  # Heartbeat every 10s while the server is alive; exit if it dies.
  while kill -0 "$SRV_PID" 2>/dev/null; do
    echo "JCODE_PROGRESS {\"percent\":100,\"message\":\"vautr-server pid $SRV_PID\"}"
    sleep 10
  done
  echo "JCODE_PROGRESS {\"percent\":0,\"message\":\"server exited; restarting\"}"
  wait "$SRV_PID" || true
  sleep 1
done
