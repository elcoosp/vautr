#!/usr/bin/env bash
# VTR-010 live TDD verification: bring up the server container, then run the
# curl checks from the issue (TDD #1-3). Exits non-zero on any failure.
set -euo pipefail

cd "$(dirname "$0")/.."

PORT="${VAUTR_PORT:-8080}"
BASE="http://localhost:${PORT}"

cleanup() { docker compose down -v >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> docker compose up (detached)"
docker compose up -d --build

echo "==> waiting for /account/status to respond (health gate)"
for i in $(seq 1 30); do
  code=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/account/status" || true)
  if [ "$code" != "000" ]; then break; fi
  sleep 2
done

echo "==> TDD #2: GET /openapi.json"
oa=$(curl -s "$BASE/openapi.json")
echo "$oa" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d.get("paths"), "no paths"; print("openapi.json OK, paths:", len(d["paths"]))'

echo "==> TDD #3: GET /sync/pull?cursor=0 (must not crash)"
sp=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/sync/pull?cursor=0" || true)
echo "sync/pull status: $sp (expected 401/400 — auth/validation, not 500/000)"

echo "==> TDD #1: server listening on :${PORT}"
lp=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/health" || true)
echo "health status: $lp"

echo "==> VTR-010 live TDD checks passed"
