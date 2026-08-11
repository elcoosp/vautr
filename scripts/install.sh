#!/usr/bin/env bash
#
# Vautr — one-command self-host installer (Wave Ops).
#
# Builds the vautr-server binary, creates a config/environment file, provisions
# an HTTPS reverse proxy with automatic Let's Encrypt certificates (Caddy by
# default, nginx+certbot optional), installs a service manager unit, and starts
# the server. SQLite is the default database; pass --db-url for PostgreSQL.
#
#   ./scripts/install.sh --domain vault.example.com --email admin@example.com
#
# Written in portable POSIX sh so it can be reviewed and executed under any
# sh-family shell. See docs/SELF-HOSTING.md for the full manual.

set -eu

# ---------------------------------------------------------------------------
# Configuration (overridable via flags; see usage()).
# ---------------------------------------------------------------------------
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

VAUTR_PORT="8080"
VAUTR_DOMAIN=""
VAUTR_EMAIL=""              # Let's Encrypt account email (required for certs)
VAUTR_DB_URL="sqlite:vautr.db"
VAUTR_DATA_DIR=""
VAUTR_RUNTIME_USER=""
VAUTR_PROXY="caddy"         # caddy | nginx | none
VAUTR_BUILD="1"             # 1 = build from source
VAUTR_BIN=""
VAUTR_CONF_FILE=""          # env file for the service unit
VAUTR_SERVICE_NAME="vautr-server"
VAUTR_NO_ROOT=""            # run as current user instead of creating vautr user

# Colours (best-effort; degrade gracefully when not a TTY).
if [ -t 1 ]; then
    CINFO="\033[1;36m"; COK="\033[1;32m"; CWARN="\033[1;33m"; CERR="\033[1;31m"; CRST="\033[0m"
else
    CINFO=""; COK=""; CWARN=""; CERR=""; CRST=""
fi

info()  { printf '%b%s%b\n' "${CINFO}[vautr]${CRST}" " $*"; }
ok()    { printf '%b%s%b\n' "${COK}[ ok ]${CRST}" " $*"; }
warn()  { printf '%b%s%b\n' "${CWARN}[warn]${CRST}" " $*" >&2; }
err()   { printf '%b%s%b\n' "${CERR}[fail]${CRST}" " $*" >&2; exit 1; }

usage() {
    cat <<'EOF'
Vautr self-host installer — one command, HTTPS + Let's Encrypt included.

Usage:
  ./scripts/install.sh [options]

Options:
  --domain <fqdn>       Public hostname to serve on (e.g. vault.example.com).
                        Required for automatic HTTPS (Let's Encrypt).
  --email <addr>        Let's Encrypt account email. Required with --domain.
  --port <port>         Local port the server binds to (default: 8080).
  --db-url <url>        Database URL (default: sqlite:vautr.db).
                        Postgres example:
                        postgres://user:pass@localhost:5432/vautr
  --data-dir <dir>      Where to keep config/env (default: /etc/vautr as root,
                        else $HOME/.config/vautr).
  --user <name>         Runtime user to run the server as (default: vautr).
                        Only used when installing as root.
  --proxy <caddy|nginx|none>
                        Reverse proxy / TLS provider (default: caddy).
                        caddy  = Caddy + automatic Let's Encrypt.
                        nginx  = nginx + certbot (auto-renews via systemd timer).
                        none   = raw server, no proxy (use behind existing TLS).
  --no-build            Use an existing binary instead of building from source
                        (honours --bin / $VAUTR_SERVER_BIN).
  --bin <path>          Path to a prebuilt vautr-server binary (with --no-build).
  -h, --help            Show this help.

Examples:
  # Quick local run, no proxy, default SQLite
  ./scripts/install.sh --no-build --proxy none

  # Production: HTTPS on vault.example.com
  ./scripts/install.sh --domain vault.example.com --email you@example.com
EOF
    exit 0
}

# ---------------------------------------------------------------------------
# Flag parsing.
# ---------------------------------------------------------------------------
while [ "$#" -gt 0 ]; do
    case "$1" in
        --domain) VAUTR_DOMAIN="${2:-}"; shift 2 ;;
        --email)  VAUTR_EMAIL="${2:-}";  shift 2 ;;
        --port)   VAUTR_PORT="${2:-}";   shift 2 ;;
        --db-url) VAUTR_DB_URL="${2:-}"; shift 2 ;;
        --data-dir) VAUTR_DATA_DIR="${2:-}"; shift 2 ;;
        --user)   VAUTR_RUNTIME_USER="${2:-}"; shift 2 ;;
        --proxy)  VAUTR_PROXY="${2:-}";  shift 2 ;;
        --bin)    VAUTR_BIN="${2:-}"; VAUTR_BUILD="0"; shift 2 ;;
        --no-build) VAUTR_BUILD="0"; shift ;;
        --no-root) VAUTR_NO_ROOT="1"; shift ;;
        -h|--help) usage ;;
        *) err "Unknown option: $1 (run --help)";;
    esac
done

# ---------------------------------------------------------------------------
# Sanity checks.
# ---------------------------------------------------------------------------
case "$VAUTR_PROXY" in
    caddy|nginx|none) ;;
    *) err "--proxy must be one of: caddy, nginx, none (got '$VAUTR_PROXY')" ;;
esac

if [ "$VAUTR_PROXY" != "none" ] && [ -z "$VAUTR_DOMAIN" ]; then
    err "--proxy $VAUTR_PROXY requires --domain <fqdn> so Let's Encrypt can issue a certificate"
fi
if [ "$VAUTR_PROXY" = "nginx" ] && [ -z "$VAUTR_EMAIL" ]; then
    warn "--proxy nginx uses certbot; you should pass --email for account registration"
fi

# Root vs non-root defaults.
if [ "$(id -u)" -eq 0 ] && [ -z "$VAUTR_NO_ROOT" ]; then
    IS_ROOT=1
else
    IS_ROOT=0
fi

if [ -z "$VAUTR_DATA_DIR" ]; then
    if [ "$IS_ROOT" -eq 1 ]; then
        VAUTR_DATA_DIR="/etc/vautr"
    else
        VAUTR_DATA_DIR="$HOME/.config/vautr"
    fi
fi

if [ -z "$VAUTR_RUNTIME_USER" ]; then
    VAUTR_RUNTIME_USER="vautr"
fi

if [ -z "$VAUTR_BIN" ]; then
    VAUTR_BIN="$REPO_ROOT/target/release/vautr-server"
fi

# ---------------------------------------------------------------------------
# Build the server binary.
# ---------------------------------------------------------------------------
build_server() {
    if [ "$VAUTR_BUILD" -ne 1 ]; then
        if [ ! -x "$VAUTR_BIN" ]; then
            err "No executable found at '$VAUTR_BIN' (use --no-build only with --bin, or drop --no-build)"
        fi
        ok "Using existing binary: $VAUTR_BIN"
        return 0
    fi
    if ! command -v cargo >/dev/null 2>&1; then
        err "cargo not found. Install a Rust toolchain first (https://rustup.rs) or use --no-build --bin <path>."
    fi
    info "Building vautr-server (release). This may take a few minutes..."
    ( cd "$REPO_ROOT" && cargo build --release -p vautr-server )
    [ -x "$VAUTR_BIN" ] || err "Build finished but '$VAUTR_BIN' is missing"
    ok "Built vautr-server: $VAUTR_BIN"
}

# ---------------------------------------------------------------------------
# Runtime user (only when installing as root).
# ---------------------------------------------------------------------------
ensure_runtime_user() {
    if [ "$IS_ROOT" -ne 1 ]; then
        VAUTR_RUNTIME_USER="$(id -un)"
        ok "Running as non-root user '$VAUTR_RUNTIME_USER' (no dedicated system user created)"
        return 0
    fi
    if ! id -u "$VAUTR_RUNTIME_USER" >/dev/null 2>&1; then
        # -r system account, -M no home, -s no login shell (best-effort per distro).
        useradd --system --home "$VAUTR_DATA_DIR" --shell /usr/sbin/nologin \
            "$VAUTR_RUNTIME_USER" 2>/dev/null \
            || adduser --system --no-create-home --shell /usr/sbin/nologin \
                "$VAUTR_RUNTIME_USER" 2>/dev/null \
            || err "could not create runtime user '$VAUTR_RUNTIME_USER'"
        ok "Created system user '$VAUTR_RUNTIME_USER'"
    fi
}

# ---------------------------------------------------------------------------
# Config / env file + data directory.
# ---------------------------------------------------------------------------
write_env() {
    mkdir -p "$VAUTR_DATA_DIR"

    # SQLite default: point at a stable path inside the data dir unless the
    # user supplied an explicit absolute/relative DB URL.
    local_db_url="$VAUTR_DB_URL"
    case "$local_db_url" in
        sqlite:*)
            case "$local_db_url" in
                sqlite::memory:*) ;;
                *)
                    # Rewrite bare sqlite:vautr.db / relative paths to the data dir.
                    path="${local_db_url#sqlite:}"
                    case "$path" in
                        /*) ;; # absolute already
                        *)
                            local_db_url="sqlite:$VAUTR_DATA_DIR/${path#sqlite:}"
                            [ "${path#sqlite:}" = "" ] && local_db_url="sqlite:$VAUTR_DATA_DIR/vautr.db"
                            ;;
                    esac
                    ;;
            esac
            ;;
    esac
    VAUTR_DB_URL="$local_db_url"

    # Write an env file the service unit sources. Keep it 0600 (holds secrets).
    VAUTR_CONF_FILE="$VAUTR_DATA_DIR/vautr.env"
    {
        printf 'VAUTR_DB_URL=%s\n' "$VAUTR_DB_URL"
        printf 'VAUTR_PORT=%s\n'   "$VAUTR_PORT"
        printf 'RUST_LOG=info\n'
    } > "$VAUTR_CONF_FILE"
    chmod 600 "$VAUTR_CONF_FILE"

    if [ "$IS_ROOT" -eq 1 ]; then
        chown -R "$VAUTR_RUNTIME_USER:$VAUTR_RUNTIME_USER" "$VAUTR_DATA_DIR"
    fi
    ok "Wrote config: $VAUTR_CONF_FILE"
}

# ---------------------------------------------------------------------------
# Reverse proxy + TLS.
# ---------------------------------------------------------------------------
install_caddy() {
    if command -v caddy >/dev/null 2>&1; then
        ok "Caddy already installed: $(caddy version 2>/dev/null || echo present)"
        return 0
    fi
    info "Installing Caddy..."
    if command -v apt-get >/dev/null 2>&1; then
        # Official Caddy apt repo (Debian/Ubuntu).
        apt-get update -y
        apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
            | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
        curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
            | tee /etc/apt/sources.list.d/caddy-stable.list
        apt-get update -y
        apt-get install -y caddy
    else
        # Fallback: download the static binary.
        caddy_tmp="$VAUTR_DATA_DIR/caddy_download"
        mkdir -p "$caddy_tmp"
        curl -fsSL "https://caddyserver.com/api/download?os=$(uname -s | tr 'A-Z' 'a-z')&arch=$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')" \
            -o "$caddy_tmp/caddy"
        chmod +x "$caddy_tmp/caddy"
        install -m 0755 "$caddy_tmp/caddy" /usr/local/bin/caddy
        rm -rf "$caddy_tmp"
    fi
    ok "Caddy installed."
}

write_caddyfile() {
    caddy_dir="/etc/caddy"
    [ "$IS_ROOT" -eq 0 ] && caddy_dir="$VAUTR_DATA_DIR"
    mkdir -p "$caddy_dir"
    caddyfile="$caddy_dir/Caddyfile"
    {
        printf '# Managed by scripts/install.sh (Vautr). Auto-generated.\n'
        printf '%s {\n' "$VAUTR_DOMAIN"
        printf '    encode gzip\n'
        printf '    reverse_proxy 127.0.0.1:%s\n' "$VAUTR_PORT"
        printf '}\n'
    } > "$caddyfile"
    ok "Wrote Caddyfile: $caddyfile"
    # Tell the unit where the Caddyfile lives.
    VAUTR_CADDYFILE="$caddyfile"
}

install_nginx() {
    info "Installing nginx + certbot..."
    if command -v apt-get >/dev/null 2>&1; then
        apt-get update -y
        apt-get install -y nginx certbot python3-certbot-nginx
    elif command -v dnf >/dev/null 2>&1; then
        dnf install -y nginx certbot python3-certbot-nginx
    elif command -v yum >/dev/null 2>&1; then
        yum install -y nginx certbot python3-certbot-nginx
    else
        err "Unsupported package manager for nginx+certbot; use --proxy caddy or --proxy none"
    fi
    nginx_site="/etc/nginx/sites-available/vautr"
    [ -d /etc/nginx/sites-available ] || nginx_site="/etc/nginx/conf.d/vautr.conf"

    {
        printf 'server {\n'
        printf '    listen 80;\n'
        printf '    server_name %s;\n' "$VAUTR_DOMAIN"
        printf '    location /.well-known/acme-challenge/ { root /var/www/html; }\n'
        printf '    location / { return 301 https://$host$request_uri; }\n'
        printf '}\n'
        printf 'server {\n'
        printf '    listen 443 ssl http2;\n'
        printf '    server_name %s;\n' "$VAUTR_DOMAIN"
        printf '    client_max_body_size 10m;\n'
        printf '    location / {\n'
        printf '        proxy_pass http://127.0.0.1:%s;\n' "$VAUTR_PORT"
        printf '        proxy_set_header Host $host;\n'
        printf '        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n'
        printf '        proxy_set_header X-Forwarded-Proto $scheme;\n'
        printf '    }\n'
        printf '}\n'
    } > "$nginx_site"

    if [ -d /etc/nginx/sites-available ] && [ -f /etc/nginx/sites-available/vautr ]; then
        ln -sf "$nginx_site" /etc/nginx/sites-enabled/vautr
    fi
    nginx -t || warn "nginx -t reported issues; review $nginx_site"
    systemctl enable nginx >/dev/null 2>&1 || true
    systemctl restart nginx >/dev/null 2>&1 || true

    # Obtain cert via certbot (webroot on :80 first, then install to nginx).
    if [ -n "$VAUTR_EMAIL" ]; then
        certbot --nginx -d "$VAUTR_DOMAIN" --non-interactive --agree-tos \
            -m "$VAUTR_EMAIL" --redirect || warn "certbot failed; run it manually"
    else
        certbot --nginx -d "$VAUTR_DOMAIN" --non-interactive --register-unsafely-without-email \
            --redirect || warn "certbot failed; run it manually"
    fi
    ok "nginx configured with Let's Encrypt for $VAUTR_DOMAIN"
}

provision_proxy() {
    case "$VAUTR_PROXY" in
        none)
            ok "--proxy none: no reverse proxy installed. Terminate TLS yourself."
            VAUTR_CADDYFILE=""
            ;;
        caddy)
            install_caddy
            write_caddyfile
            if [ "$IS_ROOT" -eq 1 ]; then
                systemctl enable caddy >/dev/null 2>&1 || true
                systemctl restart caddy >/dev/null 2>&1 || true
                ok "Caddy started. Certificate for $VAUTR_DOMAIN will be issued on first request."
            else
                warn "Non-root install: start Caddy yourself with: caddy run --config $VAUTR_CADDYFILE"
            fi
            ;;
        nginx)
            install_nginx
            ;;
    esac
}

# ---------------------------------------------------------------------------
# Service manager unit (systemd on Linux, launchd on macOS).
# ---------------------------------------------------------------------------
install_service_linux() {
    unit="/etc/systemd/system/$VAUTR_SERVICE_NAME.service"
    {
        printf '[Unit]\n'
        printf 'Description=Vautr server (password & secrets manager)\n'
        printf 'After=network-online.target\n'
        printf 'Wants=network-online.target\n\n'
        printf '[Service]\n'
        if [ "$IS_ROOT" -eq 1 ]; then
            printf 'User=%s\n' "$VAUTR_RUNTIME_USER"
            printf 'Group=%s\n' "$VAUTR_RUNTIME_USER"
        fi
        printf 'Type=simple\n'
        printf 'WorkingDirectory=%s\n' "$VAUTR_DATA_DIR"
        printf 'EnvironmentFile=%s\n' "$VAUTR_CONF_FILE"
        printf 'ExecStart=%s\n' "$VAUTR_BIN"
        printf 'Restart=on-failure\n'
        printf 'RestartSec=3\n'
        printf 'LimitNOFILE=65536\n\n'
        printf '[Install]\n'
        printf 'WantedBy=multi-user.target\n'
    } > "$unit"
    [ "$IS_ROOT" -eq 1 ] && chown root:root "$unit"
    ok "Wrote systemd unit: $unit"

    if [ "$IS_ROOT" -eq 1 ]; then
        systemctl daemon-reload
        systemctl enable "$VAUTR_SERVICE_NAME" >/dev/null 2>&1 || true
        systemctl restart "$VAUTR_SERVICE_NAME"
        ok "vautr-server started via systemd. Status: systemctl status $VAUTR_SERVICE_NAME"
    else
        warn "Non-root install: start manually with: $VAUTR_BIN (env from $VAUTR_CONF_FILE)"
    fi
}

install_service_macos() {
    plist_dir="$HOME/Library/LaunchAgents"
    mkdir -p "$plist_dir"
    plist="$plist_dir/org.vautr.server.plist"
    {
        printf '<?xml version="1.0" encoding="UTF-8"?>\n'
        printf '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
        printf '<plist version="1.0"><dict>\n'
        printf '  <key>Label</key><string>org.vautr.server</string>\n'
        printf '  <key>ProgramArguments</key><array>\n'
        printf '    <string>%s</string>\n' "$VAUTR_BIN"
        printf '  </array>\n'
        printf '  <key>WorkingDirectory</key><string>%s</string>\n' "$VAUTR_DATA_DIR"
        printf '  <key>EnvironmentVariables</key><dict>\n'
        printf '    <key>VAUTR_DB_URL</key><string>%s</string>\n' "$VAUTR_DB_URL"
        printf '    <key>RUST_LOG</key><string>info</string>\n'
        printf '  </dict>\n'
        printf '  <key>RunAtLoad</key><true/>\n'
        printf '  <key>KeepAlive</key><true/>\n'
        printf '  <key>StandardOutPath</key><string>%s/server.log</string>\n' "$VAUTR_DATA_DIR"
        printf '  <key>StandardErrorPath</key><string>%s/server.err.log</string>\n' "$VAUTR_DATA_DIR"
        printf '</dict></plist>\n'
    } > "$plist"
    ok "Wrote launchd plist: $plist"
    launchctl unload "$plist" >/dev/null 2>&1 || true
    launchctl load "$plist"
    ok "vautr-server started via launchd."
}

install_service() {
    case "$(uname -s)" in
        Linux) install_service_linux ;;
        Darwin) install_service_macos ;;
        *)
            warn "Unsupported OS for a service unit; start manually:"
            warn "  $VAUTR_BIN (env from $VAUTR_CONF_FILE)"
            ;;
    esac
}

# ---------------------------------------------------------------------------
# Final summary + smoke check.
# ---------------------------------------------------------------------------
smoke() {
    info "Waiting for server to accept connections..."
    i=0
    while [ "$i" -lt 30 ]; do
        code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$VAUTR_PORT/account/status" 2>/dev/null || echo 000)
        if [ "$code" != "000" ]; then
            ok "Server responded on :$VAUTR_PORT (HTTP $code, auth-gated endpoint)."
            return 0
        fi
        i=$((i + 1))
        sleep 1
    done
    warn "Server did not respond on :$VAUTR_PORT yet — check logs with: journalctl -u $VAUTR_SERVICE_NAME -f"
}

summary() {
    printf '\n'
    printf '  %bVautr installed successfully.%b\n' "${COK}" "${CRST}"
    printf '  - Server binary : %s\n' "$VAUTR_BIN"
    printf '  - Config        : %s\n' "$VAUTR_CONF_FILE"
    printf '  - Database      : %s\n' "$VAUTR_DB_URL"
    if [ "$VAUTR_PROXY" != "none" ]; then
        printf '  - HTTPS URL     : https://%s\n' "$VAUTR_DOMAIN"
    else
        printf '  - Local URL     : http://127.0.0.1:%s (no proxy)\n' "$VAUTR_PORT"
    fi
    printf '\nNext steps:\n'
    printf '  - Open the vault in your browser.\n'
    printf '  - Backups / Postgres / updates: see docs/SELF-HOSTING.md.\n'
    printf '\n'
}

# ---------------------------------------------------------------------------
# Main.
# ---------------------------------------------------------------------------
main() {
    info "Vautr installer"
    info "Repo: $REPO_ROOT"
    build_server
    ensure_runtime_user
    write_env
    provision_proxy
    install_service
    smoke
    summary
}

main "$@"
