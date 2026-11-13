#!/usr/bin/env bash
# Feather Chat - one-command server installer.
#
# Made for a fresh Debian/Ubuntu VPS, and for an assistant working on that VPS.
# It installs the tiny headless hub, keeps it running with systemd, and can put
# Caddy in front for automatic HTTPS. Docker is optional, not required.
#
#   sudo bash deploy/install.sh deploy --domain chat.example.com --email you@example.com
#   bash deploy/install.sh status
#   sudo bash deploy/install.sh backup
#   sudo bash deploy/install.sh uninstall
#
# Run with --help for every option.
#
# Design rules:
#   * safe to run twice (it repairs instead of duplicating),
#   * every failure stops and says what to do next,
#   * messages are for a person or an AI reading the terminal.

set -Eeuo pipefail

VERSION="1.1.0"
SERVICE_NAME="feather"
DEFAULT_PORT=8777
DATA_DIR="/var/lib/feather"
BACKUP_DIR="/var/backups/feather"
INSTALL_BIN="/usr/local/bin/feather-hub"
UNIT_FILE="/etc/systemd/system/feather.service"
CADDYFILE="/etc/caddy/Caddyfile"

SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd -- "$SELF_DIR/.." && pwd)"

ACTION="deploy"
MODE="auto"
DOMAIN=""
EMAIL=""
PORT="$DEFAULT_PORT"
SIGNUP="open"
PROXY="yes"
ASSUME_YES=0
PURGE=0
RESTORE_FILE=""

STEP=0
STEP_TOTAL=7
ORIGINAL_ARGS=("$@")

# ---------------------------------------------------------------- small tools

if [ -t 1 ]; then
    C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'; C_DIM=$'\033[2m'
    C_GREEN=$'\033[32m'; C_YELLOW=$'\033[33m'; C_RED=$'\033[31m'; C_BLUE=$'\033[34m'
else
    C_RESET=""; C_BOLD=""; C_DIM=""; C_GREEN=""; C_YELLOW=""; C_RED=""; C_BLUE=""
fi

log()  { printf '%s\n' "$*"; }
step() { STEP=$((STEP + 1)); printf '\n%s[%s/%s]%s %s%s%s\n' "$C_BLUE" "$STEP" "$STEP_TOTAL" "$C_RESET" "$C_BOLD" "$*" "$C_RESET"; }
ok()   { printf '  %sok%s   %s\n' "$C_GREEN" "$C_RESET" "$*"; }
note() { printf '  %s·%s    %s\n' "$C_DIM" "$C_RESET" "$*"; }
warn() { printf '  %s!%s    %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
die()  { printf '\n%serror:%s %s\n' "$C_RED" "$C_RESET" "$*" >&2; exit 1; }

on_error() {
    local code=$?
    printf '\n%serror:%s stopped at line %s (exit %s).\n' "$C_RED" "$C_RESET" "$1" "$code" >&2
    printf 'Useful next commands:\n' >&2
    printf '  systemctl status %s --no-pager\n' "$SERVICE_NAME" >&2
    printf '  journalctl -u %s -n 60 --no-pager\n' "$SERVICE_NAME" >&2
    printf 'Run "bash deploy/install.sh status" to see what is up.\n' >&2
}
trap 'on_error $LINENO' ERR

usage() {
    cat <<'TEXT'
Feather Chat server installer

Usage:
  sudo bash deploy/install.sh [deploy] [options]
  bash deploy/install.sh status
  sudo bash deploy/install.sh backup
  sudo bash deploy/install.sh restore --file /var/backups/feather/feather-....json
  sudo bash deploy/install.sh uninstall [--purge]

Actions:
  deploy      Install or repair the hub (default).
  status      Show whether the hub and HTTPS are healthy; exits 1 if not.
  backup      Copy the one data file into /var/backups/feather.
  restore     Stop the hub, put a backup back, start it again.
  uninstall   Stop and remove the hub; keeps messages unless --purge.

Options:
  --domain NAME       Public name, for example chat.example.com.
                      Without it the hub is served on a plain port.
  --email ADDRESS     Contact address for the HTTPS certificate (optional).
  --signup open|closed  Whether strangers can make accounts (default open).
  --mode auto|docker|native
                      auto uses Docker when it is already installed,
                      otherwise a native systemd service (default auto).
  --port N            Local port for the hub (default 8777).
  --no-proxy          Do not install Caddy or HTTPS.
  --no-docker         Same as --mode native.
  --data-dir PATH     Where hub.json lives (default /var/lib/feather).
  --yes               Do not ask before installing packages.
  --purge             With uninstall: also delete messages and the user.
  -h, --help          This text.

Examples:
  sudo bash deploy/install.sh deploy --domain chat.example.com --email me@example.com
  sudo bash deploy/install.sh deploy --mode native --domain chat.example.com
  sudo bash deploy/install.sh deploy --no-proxy --domain 203.0.113.10
  bash deploy/install.sh status
TEXT
}

need_value() {
    [ "$#" -ge 2 ] && [ -n "$2" ] || die "$1 needs a value."
}

is_ipv4() {
    [[ "$1" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]]
}

normalise_domain() {
    local d="$1"
    d="${d#http://}"; d="${d#https://}"; d="${d%%/*}"; d="${d%.}"
    printf '%s' "$d" | tr '[:upper:]' '[:lower:]'
}

require_root() {
    if [ "$(id -u)" -eq 0 ]; then return; fi
    [ "${FEATHER_INSTALL_ROOT:-0}" = "1" ] && die "This action needs root."
    command -v sudo >/dev/null 2>&1 || die "Run this as root (sudo is not installed)."
    log "This action changes the system, so it will ask for your sudo password."
    FEATHER_INSTALL_ROOT=1 exec sudo -E bash "$SELF_DIR/$(basename "${BASH_SOURCE[0]}")" "${ORIGINAL_ARGS[@]}"
}

compose() {
    if docker compose version >/dev/null 2>&1; then
        docker compose "$@"
    elif command -v docker-compose >/dev/null 2>&1; then
        docker-compose "$@"
    else
        die "Docker is here but the compose plugin is missing. Install docker-compose-plugin, or use --mode native."
    fi
}

wait_for() {
    # wait_for <seconds> <description> <command...>
    local tries="$1" what="$2"; shift 2
    local i
    for ((i = 1; i <= tries; i++)); do
        if "$@" >/dev/null 2>&1; then return 0; fi
        sleep 1
    done
    return 1
}

health_local() {
    curl -fsS --max-time 3 "http://127.0.0.1:${PORT}/api/health" >/dev/null 2>&1
}

# A brand-new hub only writes its file on the first change. Seed an empty store
# so "backup" works immediately after install and a restore is never empty.
EMPTY_STORE='{"users":[],"convs":[],"msgs":[],"tokens":{},"next_id":1,"next_msg":1,"rev":1}'

seed_store_native() {
    [ -s "${DATA_DIR}/hub.json" ] && return 0
    printf '%s\n' "$EMPTY_STORE" >"${DATA_DIR}/hub.json"
    chown feather:feather "${DATA_DIR}/hub.json" 2>/dev/null || true
    chmod 600 "${DATA_DIR}/hub.json"
}

seed_store_docker() {
    ( cd "$APP_DIR" && compose exec -T hub test -s /data/hub.json ) >/dev/null 2>&1 && return 0
    printf '%s\n' "$EMPTY_STORE" | ( cd "$APP_DIR" && compose exec -T hub sh -c 'cat > /data/hub.json' ) >/dev/null 2>&1 || true
}

# Read the settings the installed service actually uses, so status/backup do
# not depend on the person retyping --port or --data-dir.
read_installed() {
    [ -f "$UNIT_FILE" ] || return 0
    local line
    line="$(grep -m1 '^Environment=FEATHER_PORT=' "$UNIT_FILE" 2>/dev/null || true)"
    [ -n "$line" ] && PORT="${line#*FEATHER_PORT=}"
    line="$(grep -m1 '^Environment=FEATHER_DATA=' "$UNIT_FILE" 2>/dev/null || true)"
    [ -n "$line" ] && DATA_DIR="$(dirname "${line#*FEATHER_DATA=}")"
    return 0
}

# ---------------------------------------------------------------- arguments

while [ "$#" -gt 0 ]; do
    case "$1" in
        deploy|status|backup|restore|uninstall) ACTION="$1"; shift ;;
        --domain) need_value "$@"; DOMAIN="$(normalise_domain "$2")"; shift 2 ;;
        --email) need_value "$@"; EMAIL="$2"; shift 2 ;;
        --port) need_value "$@"; PORT="$2"; shift 2 ;;
        --signup) need_value "$@"; SIGNUP="$2"; shift 2 ;;
        --mode) need_value "$@"; MODE="$2"; shift 2 ;;
        --no-proxy) PROXY="no"; shift ;;
        --no-docker) MODE="native"; shift ;;
        --data-dir) need_value "$@"; DATA_DIR="$2"; shift 2 ;;
        --file) need_value "$@"; RESTORE_FILE="$2"; shift 2 ;;
        --yes|-y) ASSUME_YES=1; shift ;;
        --purge) PURGE=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) die "Unknown option: $1 (try --help)." ;;
    esac
done

case "$SIGNUP" in open|closed) ;; *) die "--signup must be open or closed." ;; esac
case "$MODE" in auto|docker|native) ;; *) die "--mode must be auto, docker or native." ;; esac
[[ "$PORT" =~ ^[0-9]+$ ]] || die "--port must be a number."
[ -n "$DOMAIN" ] || DOMAIN=""

if [ "$ACTION" = "deploy" ]; then
    if [ -n "$DOMAIN" ] && is_ipv4 "$DOMAIN"; then
        if [ "$PROXY" = "yes" ]; then
            warn "A bare IP address cannot get a normal HTTPS certificate here; continuing without the proxy."
            PROXY="no"
        fi
    fi
    if [ -z "$DOMAIN" ] && [ "$PROXY" = "yes" ]; then
        warn "No --domain given, so the hub will run without Caddy. Use --domain for HTTPS."
        PROXY="no"
    fi
fi

# ---------------------------------------------------------------- actions

show_status() {
    local failed=0
    log ""
    log "${C_BOLD}Feather status${C_RESET}  (${APP_DIR})"
    log ""

    if systemctl list-unit-files "${SERVICE_NAME}.service" >/dev/null 2>&1 \
        && systemctl cat "${SERVICE_NAME}.service" >/dev/null 2>&1; then
        local state
        state="$(systemctl is-active "${SERVICE_NAME}.service" 2>/dev/null || true)"
        log "  service: ${state:-unknown}"
        [ "$state" = "active" ] || failed=1
        if command -v feather-hub >/dev/null 2>&1; then
            log "  version: $(feather-hub --version 2>/dev/null || echo '?')"
        fi
        log "  data:    ${DATA_DIR}/hub.json"
        if [ -f "${DATA_DIR}/hub.json" ]; then
            log "  size:    $(du -h "${DATA_DIR}/hub.json" | cut -f1)"
        fi
        health_local && log "  health:  local hub answers" || { log "  health:  NO ANSWER"; failed=1; }
    elif [ -f "${APP_DIR}/docker-compose.yml" ] && command -v docker >/dev/null 2>&1 \
        && docker inspect feather-hub-1 >/dev/null 2>&1; then
        log "  service: docker"
        local health
        health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' feather-hub-1 2>/dev/null || echo unknown)"
        log "  health:  docker says ${health}"
        if compose exec -T hub feather-hub --health >/dev/null 2>&1; then
            log "  hub:     answers inside the container"
            [ "$health" = "unhealthy" ] && failed=1
        else
            log "  hub:     NO ANSWER"
            failed=1
        fi
        log "  data:    docker volume feather_data"
    else
        log "  service: not installed here"
        failed=1
    fi

    if [ "$failed" -eq 0 ] && [ -n "$DOMAIN" ] && [ "$PROXY" = "yes" ]; then
        local code insecure=()
        [ "$DOMAIN" = "localhost" ] && insecure=(-k)
        code="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 \
            "${insecure[@]}" --resolve "${DOMAIN}:443:127.0.0.1" \
            "https://${DOMAIN}/api/health" 2>/dev/null || true)"
        log "  https:   ${code:-no answer}  https://${DOMAIN}"
        [ "$code" = "200" ] || failed=1
    fi

    if [ -n "$DOMAIN" ] && [ "$PROXY" = "yes" ]; then
        log "  address: https://${DOMAIN}"
    elif [ -n "$DOMAIN" ]; then
        log "  address: http://${DOMAIN}:${PORT}"
    fi

    log ""
    if [ "$failed" -eq 0 ]; then
        ok "Everything asked for is healthy."
        return 0
    fi
    warn "Something is not healthy. See the commands below."
    if command -v docker >/dev/null 2>&1 && docker inspect feather-hub-1 >/dev/null 2>&1; then
        log "  cd ${APP_DIR} && docker compose logs --tail 60"
        log "  cd ${APP_DIR} && docker compose ps"
    else
        log "  journalctl -u ${SERVICE_NAME} -n 60 --no-pager"
    fi
    return 1
}

do_backup() {
    require_root "$@"
    read_installed
    [ -d "$DATA_DIR" ] || [ -f "${APP_DIR}/docker-compose.yml" ] || die "No Feather install found here."
    mkdir -p "$BACKUP_DIR"
    chmod 700 "$BACKUP_DIR"
    local stamp file n
    stamp="$(date +%Y-%m-%d-%H%M%S)"
    file="${BACKUP_DIR}/feather-${stamp}.json"
    n=1
    while [ -e "$file" ]; do
        file="${BACKUP_DIR}/feather-${stamp}-${n}.json"
        n=$((n + 1))
    done

    if systemctl cat "${SERVICE_NAME}.service" >/dev/null 2>&1; then
        [ -f "${DATA_DIR}/hub.json" ] || die "No message file at ${DATA_DIR}/hub.json yet."
        install -m 600 "${DATA_DIR}/hub.json" "$file"
    elif command -v docker >/dev/null 2>&1 && docker inspect feather-hub-1 >/dev/null 2>&1; then
        ( cd "$APP_DIR" && compose exec -T hub cat /data/hub.json ) >"$file"
        chmod 600 "$file"
        [ -s "$file" ] || die "The container returned an empty message file."
    else
        die "No running Feather found to back up."
    fi

    # Keep the newest 14; a small circle does not need a warehouse.
    ls -1t "${BACKUP_DIR}"/feather-*.json 2>/dev/null | tail -n +15 | while read -r old; do rm -f -- "$old"; done || true
    ok "Backup written: $file"
    note "Copy it somewhere else now and then. It contains every message."
}

do_restore() {
    require_root "$@"
    read_installed
    [ -n "$RESTORE_FILE" ] || die "restore needs --file /path/to/feather-....json"
    [ -f "$RESTORE_FILE" ] || die "No such backup: $RESTORE_FILE"
    head -c 1 "$RESTORE_FILE" | grep -q '{' || die "That file does not look like a Feather message file."
    if command -v python3 >/dev/null 2>&1; then
        python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$RESTORE_FILE" \
            || die "That backup is not valid JSON."
    fi

    if systemctl cat "${SERVICE_NAME}.service" >/dev/null 2>&1; then
        systemctl stop "$SERVICE_NAME"
        install -m 600 "$RESTORE_FILE" "${DATA_DIR}/hub.json"
        chown feather:feather "${DATA_DIR}/hub.json" 2>/dev/null || true
        systemctl start "$SERVICE_NAME"
        wait_for 20 "hub" health_local || die "The hub did not come back after restore. Check journalctl -u feather."
        ok "Restored into ${DATA_DIR}/hub.json and restarted."
    elif command -v docker >/dev/null 2>&1 && docker inspect feather-hub-1 >/dev/null 2>&1; then
        ( cd "$APP_DIR" && compose stop hub >/dev/null )
        ( cd "$APP_DIR" && compose run --rm --no-deps -T --entrypoint sh hub -c 'cat > /data/hub.json' ) <"$RESTORE_FILE"
        ( cd "$APP_DIR" && compose start hub >/dev/null )
        sleep 2
        ( cd "$APP_DIR" && compose exec -T hub feather-hub --health >/dev/null ) \
            || die "The container did not come back after restore."
        ok "Restored into the Docker volume and restarted."
    else
        die "No Feather install found here."
    fi
}

do_uninstall() {
    require_root "$@"
    read_installed
    local removed=0

    if systemctl cat "${SERVICE_NAME}.service" >/dev/null 2>&1; then
        systemctl disable --now "$SERVICE_NAME" 2>/dev/null || true
        rm -f "$UNIT_FILE"
        rm -rf "${UNIT_FILE}.d"
        systemctl daemon-reload
        rm -f "$INSTALL_BIN"
        ok "Removed the systemd service and program."
        removed=1
    fi

    if command -v docker >/dev/null 2>&1 && [ -f "${APP_DIR}/docker-compose.yml" ] \
        && docker inspect feather-hub-1 >/dev/null 2>&1; then
        if [ "$PURGE" -eq 1 ]; then
            ( cd "$APP_DIR" && compose down --volumes --remove-orphans ) || true
            ok "Removed the containers and the message volume."
        else
            ( cd "$APP_DIR" && compose down --remove-orphans ) || true
            ok "Removed the containers; the message volume is kept."
        fi
        removed=1
    fi

    if [ -f "$CADDYFILE" ] && grep -q "Feather" "$CADDYFILE" 2>/dev/null; then
        if [ -f "${CADDYFILE}.feather-backup" ]; then
            mv -f "${CADDYFILE}.feather-backup" "$CADDYFILE"
            ok "Put your earlier Caddyfile back."
        else
            rm -f "$CADDYFILE"
            ok "Removed Feather's Caddyfile."
        fi
        systemctl reload caddy 2>/dev/null || systemctl restart caddy 2>/dev/null || true
    fi

    if [ "$PURGE" -eq 1 ]; then
        rm -rf "$DATA_DIR"
        userdel feather 2>/dev/null || true
        ok "Deleted the message file and the feather user."
    elif [ -d "$DATA_DIR" ]; then
        note "Messages kept in ${DATA_DIR}/hub.json."
    fi

    [ "$removed" -eq 1 ] || die "Nothing to uninstall here."
    log "Feather has been removed from this server."
}

# ---------------------------------------------------------------- deploy

preflight() {
    [ "$(uname -s)" = "Linux" ] || die "This installer is for Linux. On Windows or macOS, run the desktop app instead."
    command -v curl >/dev/null 2>&1 || die "curl is missing. Install it first: apt-get install -y curl ca-certificates"
    command -v ss >/dev/null 2>&1 || true

    if [ ! -f "${APP_DIR}/Cargo.toml" ] || [ ! -d "${APP_DIR}/core" ]; then
        die "Run this from the app folder (Cargo.toml and core/ must be next to deploy/)."
    fi

    if [ "$MODE" = "auto" ]; then
        # Repair what is already here first, then pick the lighter path.
        if systemctl cat "${SERVICE_NAME}.service" >/dev/null 2>&1; then
            MODE="native"
        elif command -v docker >/dev/null 2>&1 && docker inspect feather-hub-1 >/dev/null 2>&1; then
            MODE="docker"
        elif [ "$PROXY" = "yes" ] && command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
            MODE="docker"
        elif command -v apt-get >/dev/null 2>&1; then
            MODE="native"
        else
            MODE="docker"
        fi
    fi

    if [ "$MODE" = "docker" ] && [ "$PROXY" = "no" ]; then
        die "Docker mode always runs the HTTPS proxy. Drop --no-proxy, give a domain, or use --mode native."
    fi
    if [ "$MODE" = "native" ] && ! command -v apt-get >/dev/null 2>&1; then
        die "The native path supports Debian/Ubuntu. On this distribution use --mode docker (Docker installs its own runtime)."
    fi

    if [ "$PROXY" = "yes" ] && systemctl is-active caddy >/dev/null 2>&1 \
        && [ -f "$CADDYFILE" ] \
        && grep -Eq '^[A-Za-z0-9*_.-]+\.[A-Za-z0-9.-]+[[:space:]]*\{' "$CADDYFILE" \
        && ! grep -q "Feather" "$CADDYFILE"; then
        die "Caddy is already serving another site on this server. Feather will not overwrite it.
Add a site block yourself, or use --no-proxy --port 8778 (plain HTTP) for now."
    fi

    if [ "$PROXY" = "yes" ] || [ -n "$DOMAIN" ]; then
        if command -v ss >/dev/null 2>&1; then
            local busy own_docker=1
            busy="$(ss -ltn 2>/dev/null | awk 'NR>1 {print $4}' | grep -E ':(80|443)$' || true)"
            if command -v docker >/dev/null 2>&1 \
                && [ "$(docker inspect feather-caddy-1 --format '{{.State.Status}}' 2>/dev/null || true)" = "running" ]; then
                own_docker=0
            fi
            if [ -n "$busy" ] && ! systemctl is-active caddy >/dev/null 2>&1 && [ "$own_docker" -ne 0 ]; then
                die "Port 80 or 443 is already used by another program. Stop it first, or use --no-proxy.
Run: ss -ltnp | grep -E ':(80|443)$'"
            fi
        fi
    fi

    if [ -n "$DOMAIN" ] && [ "$PROXY" = "yes" ]; then
        if command -v getent >/dev/null 2>&1; then
            if getent ahosts "$DOMAIN" >/dev/null 2>&1; then
                note "DNS: ${DOMAIN} resolves. Make sure the address is this server."
            else
                warn "DNS: ${DOMAIN} does not resolve yet. HTTPS will wait for the certificate."
                warn "Add an A record for ${DOMAIN} pointing at this server's public IP, then rerun if needed."
            fi
        fi
    fi

    if [ "$MODE" = "docker" ]; then STEP_TOTAL=7; else STEP_TOTAL=9; fi
    note "Mode: ${MODE}; sign-ups: ${SIGNUP}; proxy: ${PROXY}"
}

install_system_packages() {
    if command -v apt-get >/dev/null 2>&1; then
        export DEBIAN_FRONTEND=noninteractive
        apt-get update -qq
        apt-get install -y -qq --no-install-recommends ca-certificates curl "$@" >/dev/null
    elif command -v dnf >/dev/null 2>&1; then
        dnf install -y -q "$@" >/dev/null
    else
        die "Unsupported Linux distribution (no apt-get or dnf). Use --mode docker on a Docker host."
    fi
}

ensure_docker() {
    if ! command -v docker >/dev/null 2>&1; then
        step "Installing Docker"
        install_system_packages ca-certificates curl
        curl -fsSL https://get.docker.com | sh
        ok "Docker installed."
    fi
    if ! docker info >/dev/null 2>&1; then
        systemctl enable --now docker >/dev/null 2>&1 || true
    fi
    wait_for 30 "Docker daemon" docker info \
        || die "Docker is installed but its service is not running (systemctl start docker)."
    compose version >/dev/null 2>&1 || die "Docker Compose is missing. Install docker-compose-plugin, or use --mode native."
}

write_env_file() {
    local env_file="${APP_DIR}/.env"
    if [ -f "$env_file" ]; then
        cp -f "$env_file" "${env_file}.bak"
        note "Saved the old .env as .env.bak"
    fi
    {
        echo "# Written by deploy/install.sh"
        echo "FEATHER_DOMAIN=${DOMAIN}"
        echo "FEATHER_SIGNUP=${SIGNUP}"
    } >"$env_file"
    chmod 600 "$env_file"
}

deploy_docker() {
    ensure_docker

    step "Writing the two settings"
    write_env_file
    ok "Domain: ${DOMAIN:-localhost}; sign-ups: ${SIGNUP}"

    step "Building the container image"
    ( cd "$APP_DIR" && compose build --pull >/dev/null )
    ok "Image built."

    step "Starting the hub and HTTPS proxy"
    ( cd "$APP_DIR" && compose up -d --remove-orphans )
    ok "Containers started."

    step "Waiting for the hub to answer"
    local i
    for ((i = 1; i <= 60; i++)); do
        if ( cd "$APP_DIR" && compose exec -T hub feather-hub --health >/dev/null 2>&1 ); then break; fi
        sleep 1
    done
    ( cd "$APP_DIR" && compose exec -T hub feather-hub --health >/dev/null 2>&1 ) \
        || die "The hub did not answer. Run: cd ${APP_DIR} && docker compose logs --tail 80 hub"
    seed_store_docker
    # Give Docker's own health check one run, so `docker compose ps` agrees.
    for ((i = 1; i <= 45; i++)); do
        local hstate
        hstate="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}running{{end}}' feather-hub-1 2>/dev/null || true)"
        [ "$hstate" = "healthy" ] && break
        sleep 1
    done
    ok "Hub is healthy."

    step "Checking HTTPS"
    if [ "$PROXY" = "yes" ]; then
        wait_for_https || warn "HTTPS is not ready yet. Caddy may still be getting the certificate; check: docker compose logs --tail 80 caddy"
    else
        note "Proxy disabled; serving plain HTTP on port ${PORT}."
    fi

    step "Making it start after a reboot"
    if systemctl enable docker >/dev/null 2>&1; then ok "Docker starts at boot; the containers restart themselves."; else warn "Could not confirm Docker at boot."; fi

    summary
}

ensure_rust() {
    if command -v cargo >/dev/null 2>&1; then return; fi
    step "Installing the Rust build tools (one time, a few minutes)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
    ok "Rust installed."
}

deploy_native() {
    command -v systemctl >/dev/null 2>&1 || die "This server has no systemd. Use --mode docker instead."
    [ -d /run/systemd/system ] || die "systemd is not running here. Use --mode docker instead."

    step "Installing the build tools"
    install_system_packages build-essential pkg-config
    ok "C compiler and tools ready."

    step "Getting Rust"
    ensure_rust
    ok "$(cargo --version)"

    step "Building the tiny hub"
    local jobs
    jobs="$(nproc 2>/dev/null || echo 1)"
    if [ -r /proc/meminfo ]; then
        local mem_mb
        mem_mb="$(awk '/MemTotal/ {print int($2/1024)}' /proc/meminfo)"
        if [ "${mem_mb:-0}" -lt 1200 ]; then
            jobs=1
            note "Low memory (${mem_mb} MB), so compiling with one job."
        fi
    fi
    ( cd "$APP_DIR" && cargo build --release -j "$jobs" -p feather-core --bin feather-hub )
    ok "Built $(du -h "${APP_DIR}/target/release/feather-hub" | cut -f1) hub."

    step "Creating the service account and data folder"
    if ! id feather >/dev/null 2>&1; then
        useradd --system --home-dir "$DATA_DIR" --shell /usr/sbin/nologin feather
    fi
    mkdir -p "$DATA_DIR"
    chown -R feather:feather "$DATA_DIR"
    chmod 750 "$DATA_DIR"
    seed_store_native
    install -m 755 "${APP_DIR}/target/release/feather-hub" "$INSTALL_BIN"
    ok "Program installed at ${INSTALL_BIN}."

    step "Writing the service"
    local bind="127.0.0.1"
    [ "$PROXY" = "yes" ] || bind="0.0.0.0"
    cat >"$UNIT_FILE" <<UNIT
[Unit]
Description=Feather chat hub
Documentation=file://${APP_DIR}/DEPLOY.md
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=feather
Group=feather
Environment=FEATHER_BIND=${bind}
Environment=FEATHER_PORT=${PORT}
Environment=FEATHER_DATA=${DATA_DIR}/hub.json
Environment=FEATHER_SIGNUP=${SIGNUP}
ExecStart=${INSTALL_BIN}
Restart=always
RestartSec=2
ReadWritePaths=${DATA_DIR}
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectControlGroups=true
RestrictSUIDSGID=true

[Install]
WantedBy=multi-user.target
UNIT
    systemctl daemon-reload
    systemctl enable "$SERVICE_NAME" >/dev/null 2>&1
    systemctl restart "$SERVICE_NAME"
    ok "Service enabled and started."

    step "Waiting for the hub to answer"
    wait_for 30 "hub" health_local \
        || die "The hub did not answer on 127.0.0.1:${PORT}. Check: journalctl -u feather -n 60 --no-pager"
    ok "Hub is healthy."

    if [ "$PROXY" = "yes" ]; then
        step "Putting Caddy in front for HTTPS"
        install_caddy
        write_caddyfile
        ok "Caddy configured for ${DOMAIN}."
        wait_for_https || warn "HTTPS is not ready yet. Caddy may still be getting the certificate; check: journalctl -u caddy -n 60 --no-pager"
    else
        step "Opening the plain port"
        note "No proxy requested, so the hub listens on 0.0.0.0:${PORT} over plain HTTP."
        open_firewall_port "$PORT"
    fi

    open_firewall_web

    step "Start on boot"
    systemctl is-enabled "$SERVICE_NAME" >/dev/null 2>&1 && ok "Feather starts automatically after a reboot."

    summary
}

install_caddy() {
    if command -v caddy >/dev/null 2>&1; then ok "Caddy already installed."; return; fi
    if ! command -v apt-get >/dev/null 2>&1; then
        die "Automatic Caddy install supports Debian/Ubuntu. Install Caddy yourself, or use --no-proxy."
    fi
    install_system_packages debian-keyring debian-archive-keyring apt-transport-https gnupg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
        | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
        >/etc/apt/sources.list.d/caddy-stable.list
    apt-get update -qq
    apt-get install -y -qq caddy >/dev/null
    ok "Caddy installed."
}

write_caddyfile() {
    mkdir -p "$(dirname "$CADDYFILE")"
    if [ -f "$CADDYFILE" ] && ! grep -q "Feather" "$CADDYFILE"; then
        cp -f "$CADDYFILE" "${CADDYFILE}.feather-backup"
        note "Saved your earlier Caddyfile as ${CADDYFILE}.feather-backup"
    fi
    cat >"$CADDYFILE" <<CADDY
# Feather - managed by deploy/install.sh
${DOMAIN} {
	encode zstd gzip

	header {
		Strict-Transport-Security "max-age=31536000; includeSubDomains"
		X-Content-Type-Options "nosniff"
		Referrer-Policy "no-referrer"
	}

	reverse_proxy 127.0.0.1:${PORT}
}
CADDY
    caddy validate --config "$CADDYFILE" >/dev/null 2>&1 || die "Caddy rejected the generated config at ${CADDYFILE}."
    systemctl enable caddy >/dev/null 2>&1 || true
    systemctl reload caddy >/dev/null 2>&1 || systemctl restart caddy
}

wait_for_https() {
    local i code insecure=()
    [ "$DOMAIN" = "localhost" ] && insecure=(-k)
    for ((i = 1; i <= 60; i++)); do
        code="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 5 \
            "${insecure[@]}" --resolve "${DOMAIN}:443:127.0.0.1" \
            "https://${DOMAIN}/api/health" 2>/dev/null || true)"
        if [ "$code" = "200" ]; then ok "HTTPS answers for https://${DOMAIN}."; return 0; fi
        sleep 2
    done
    return 1
}

open_firewall_port() {
    local port="$1"
    if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q "Status: active"; then
        ufw allow "${port}/tcp" >/dev/null 2>&1 || true
        note "Opened ${port}/tcp in ufw."
    fi
}

open_firewall_web() {
    if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q "Status: active"; then
        ufw allow 80/tcp >/dev/null 2>&1 || true
        ufw allow 443/tcp >/dev/null 2>&1 || true
        note "Opened 80 and 443 in ufw."
    fi
    if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
        firewall-cmd --permanent --add-service=http >/dev/null 2>&1 || true
        firewall-cmd --permanent --add-service=https >/dev/null 2>&1 || true
        firewall-cmd --reload >/dev/null 2>&1 || true
        note "Opened http and https in firewalld."
    fi
}

summary() {
    local address data service logs
    if [ "$MODE" = "docker" ]; then
        data="Docker volume feather_data"
        service="cd ${APP_DIR} && docker compose ps"
        logs="cd ${APP_DIR} && docker compose logs -f hub"
    else
        data="${DATA_DIR}/hub.json"
        service="systemctl status feather --no-pager"
        logs="journalctl -u feather -f"
    fi
    if [ "$MODE" = "docker" ]; then
        address="https://${DOMAIN:-localhost}"
    elif [ -n "$DOMAIN" ] && [ "$PROXY" = "yes" ]; then
        address="https://${DOMAIN}"
    elif [ -n "$DOMAIN" ]; then
        address="http://${DOMAIN}:${PORT}"
    else
        address="http://<this server's address>:${PORT}"
    fi

    log ""
    log "${C_GREEN}${C_BOLD}Feather is ready.${C_RESET}"
    log ""
    log "  Open:     ${C_BOLD}${address}${C_RESET}"
    log "  Data:     ${data}"
    log "  Service:  ${service}"
    log "  Logs:     ${logs}"
    log "  Sign-ups: ${SIGNUP}"
    log ""
    log "  Next: create the first account in a browser, then close sign-ups with:"
    if [ -n "$DOMAIN" ]; then
        log "    sudo bash deploy/install.sh deploy --mode ${MODE} --domain ${DOMAIN} --signup closed"
    else
        log "    sudo bash deploy/install.sh deploy --mode ${MODE} --signup closed"
    fi
    log "  Backups:"
    log "    sudo bash deploy/install.sh backup"
    log ""
    log "  Check any time: bash deploy/install.sh status"
    log ""
}

# ---------------------------------------------------------------- go

case "$ACTION" in
    deploy)
        require_root "$@"
        step "Checking this server"
        preflight
        if [ "$MODE" = "docker" ]; then deploy_docker; else deploy_native; fi
        ;;
    status)
        # Status is read-only, so it does not need root.
        read_installed
        if [ -z "$DOMAIN" ] && [ -f "${APP_DIR}/.env" ]; then
            DOMAIN="$(sed -n 's/^FEATHER_DOMAIN=//p' "${APP_DIR}/.env" | head -1)"
        fi
        if [ -z "$DOMAIN" ] && [ -f "$CADDYFILE" ] && grep -q "Feather" "$CADDYFILE" 2>/dev/null; then
            DOMAIN="$(awk '/^[^#[:space:]]+ \{/ {print $1; exit}' "$CADDYFILE")"
        fi
        if [ -z "$DOMAIN" ]; then PROXY="no"; fi
        show_status || exit 1
        ;;
    backup) do_backup "$@" ;;
    restore) do_restore "$@" ;;
    uninstall) do_uninstall "$@" ;;
    *) die "Unknown action: $ACTION" ;;
esac
