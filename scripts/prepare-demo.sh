#!/usr/bin/env bash
# scripts/prepare-demo.sh — robust demo bootstrap.
# NO `set -e` and NO EXIT trap — a crash in one service must NOT kill the other.
# Ctrl+C here shuts both down cleanly via the INT/TERM trap.

cd "$(dirname "$0")/.."   # repo root

LOG_DIR="${TMPDIR:-/tmp}/vautr-demo"
mkdir -p "$LOG_DIR"

echo "==> Building WASM crypto"
pnpm prepare:wasm

echo "==> Building vautr-server binary"
cargo build --bin vautr-server

echo "==> Wiping stale demo DB"
rm -f /tmp/demo.db*

echo "==> Starting vautr-server (log: $LOG_DIR/server.log)"
VAUTR_DB_URL=sqlite:/tmp/demo.db ./target/debug/vautr-server \
  >"$LOG_DIR/server.log" 2>&1 &
SERVER_PID=$!

echo "==> Starting Vite dev server (log: $LOG_DIR/web.log)"
pnpm --filter @vautr/web dev \
  >"$LOG_DIR/web.log" 2>&1 &
WEB_PID=$!

cleanup() {
  echo ""
  echo "==> Shutting down (server=$SERVER_PID, web=$WEB_PID)"
  kill "$SERVER_PID" "$WEB_PID" 2>/dev/null || true
  wait 2>/dev/null || true
  exit 0
}
trap cleanup INT TERM    # NOTE: not EXIT — only on signal

echo "==> Waiting for server /health"
for i in $(seq 1 30); do
  if curl -fsS http://localhost:8080/health >/dev/null 2>&1; then
    echo "    server up after ${i}s"
    break
  fi
  [ "$i" -eq 30 ] && { echo "    server did not come up — check $LOG_DIR/server.log"; exit 1; }
  sleep 1
done

echo ""
echo "==> Ready. In another terminal:"
echo "      pnpm rec:demo"
echo ""
echo "    Web:     http://127.0.0.1:5173/register"
echo "    Server:  http://localhost:8080/health"
echo "    Logs:    tail -f $LOG_DIR/server.log $LOG_DIR/web.log"
echo ""
echo "Press Ctrl+C here when done. Do NOT close this terminal."
echo ""

# Watchdog: warn loudly if a service dies, but DON'T take down the other.
while true; do
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "==> [$(date +%T)] *** vautr-server DIED *** — check $LOG_DIR/server.log"
    echo "    Restart with: VAUTR_DB_URL=sqlite:/tmp/demo.db ./target/debug/vautr-server &"
  fi
  if ! kill -0 "$WEB_PID" 2>/dev/null; then
    echo "==> [$(date +%T)] *** Vite dev DIED *** — check $LOG_DIR/web.log"
    echo "    Restart with: pnpm --filter @vautr/web dev &"
  fi
  sleep 5
done
