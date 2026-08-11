#!/usr/bin/env bash
#
# Vautr — no-downtime update helper (Wave Ops).
#
# Rebuilds the vautr-server binary and rolls it out with minimal downtime:
#
#   - Bare metal (systemd): builds a fresh release binary to a temp path, then
#     atomically swaps it into place and restarts the service. The server's
#     graceful shutdown (docs/architecture) drains in-flight requests, so the
#     restart is a short blip rather than a hard kill.
#   - Docker Compose: builds a new image and recreates the `server` container
#     (`docker compose up -d --build`), which starts the new container before
#     stopping the old one where possible.
#
# Usage:
#   ./scripts/update.sh                 # update the installed bare-metal service
#   ./scripts/update.sh --compose       # update via docker compose (zero-downtime)
#   ./scripts/update.sh --no-restart    # rebuild binary only, don't restart
#
# See docs/SELF-HOSTING.md §"Updating".

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

MODE="systemd"
DO_RESTART=1
BIN="$REPO_ROOT/target/release/vautr-server"

usage() {
    cat <<'EOF'
Vautr update helper.

Usage:
  ./scripts/update.sh [--compose] [--no-restart]

Options:
  --compose     Update the Docker Compose stack instead of bare metal.
  --no-restart  Rebuild the binary/image but do not restart the service.
  -h, --help    Show this help.
EOF
    exit 0
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --compose)    MODE="compose"; shift ;;
        --no-restart) DO_RESTART=0;  shift ;;
        -h|--help)    usage ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

update_compose() {
    ( cd "$REPO_ROOT" && docker compose up -d --build )
    if [ "$DO_RESTART" -eq 1 ]; then
        # Recreate only the server to apply the new image without touching data.
        ( cd "$REPO_ROOT" && docker compose up -d server )
    fi
    echo "[vautr] Compose stack updated. New image applied to 'server'."
}

update_systemd() {
    echo "[vautr] Building vautr-server (release)..."
    ( cd "$REPO_ROOT" && cargo build --release -p vautr-server )
    [ -x "$BIN" ] || { echo "[vautr] Build produced no binary at $BIN" >&2; exit 1; }

    installed_bin=""
    if command -v systemctl >/dev/null 2>&1 && systemctl is-active vautr-server >/dev/null 2>&1; then
        # Resolve the actual ExecStart of the running unit, if we can read it.
        installed_bin=$(systemctl show -p ExecStart --value vautr-server 2>/dev/null \
            | sed -E 's#^(/usr/bin/env )?##' | awk '{print $1}')
    fi
    if [ -z "$installed_bin" ] || [ "$installed_bin" = "$BIN" ]; then
        # Default layout: install.sh copied/used the release binary path directly.
        installed_bin="$BIN"
    fi

    # Build to a temp path then swap atomically so a crash mid-update never
    # leaves a half-written binary.
    tmp="$installed_bin.new.$$"
    cp "$BIN" "$tmp"
    chmod +x "$tmp"
    mv -f "$tmp" "$installed_bin"

    echo "[vautr] New binary installed at $installed_bin"
    if [ "$DO_RESTART" -eq 1 ]; then
        systemctl restart vautr-server
        echo "[vautr] Restarted vautr-server (graceful drain on shutdown)."
        systemctl --no-pager status vautr-server --lines=0 || true
    else
        echo "[vautr] --no-restart: run 'systemctl restart vautr-server' to apply."
    fi
}

case "$MODE" in
    compose) update_compose ;;
    *)       update_systemd ;;
esac
