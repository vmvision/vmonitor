#!/bin/bash

set -euo pipefail # Enable strict error handling

# Constants
readonly GITHUB_BASE_URL="https://github.com/vmvision/vmonitor"
readonly GITHUB_API_URL="https://api.github.com/repos/vmvision/vmonitor/releases/latest"
readonly GITHUB_DOWNLOAD_URL="${GITHUB_BASE_URL}/releases/download"
readonly SERVICE_FILE_URL="https://raw.githubusercontent.com/vmvision/vmonitor/refs/heads/master/release/vmonitor.service"
readonly INSTALL_DIR="/etc/vmonitor"
readonly BIN_PATH="/usr/local/bin/vmonitor"
readonly SERVICE_PATH="/etc/systemd/system/vmonitor.service"
readonly CONFIG_PATH="${INSTALL_DIR}/config.toml"
readonly TEMP_DIR=$(mktemp -d)

# Cleanup temp directory on exit
trap 'rm -rf "${TEMP_DIR}"' EXIT

# Color definitions
declare -A colors=(
    ["red"]='\033[0;31m'
    ["green"]='\033[0;32m'
    ["yellow"]='\033[0;33m'
    ["plain"]='\033[0m'
)

# Logging functions with timestamps
log_info() {
    echo -e "[$(date '+%Y-%m-%d %H:%M:%S')] ${colors[green]}[INFO]${colors[plain]} $1"
}

log_warn() {
    echo -e "[$(date '+%Y-%m-%d %H:%M:%S')] ${colors[yellow]}[WARN]${colors[plain]} $1" >&2
}

log_error() {
    echo -e "[$(date '+%Y-%m-%d %H:%M:%S')] ${colors[red]}[ERROR]${colors[plain]} $1" >&2
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi
}

detect_architecture() {
    local detected_arch
    detected_arch=$(uname -m)
    
    case "$detected_arch" in
        x86_64|x64|amd64)
            echo "x86_64-unknown-linux-gnu"
            ;;
        aarch64|arm64)
            echo "aarch64-unknown-linux-gnu"
            ;;
        armv7*)
            echo "armv7-unknown-linux-gnueabihf"
            ;;
        i686|i386)
            echo "i686-unknown-linux-gnu"
            ;;
        *)
            log_warn "Unsupported architecture: ${detected_arch}, defaulting to x86_64-unknown-linux-gnu"
            echo "x86_64-unknown-linux-gnu"
            ;;
    esac
}

detect_os() {
    local os version_id

    if [[ -f /etc/os-release ]]; then
        source /etc/os-release
        os=$ID
        version_id=$VERSION_ID
    elif [[ -f /etc/lsb-release ]]; then
        source /etc/lsb-release
        os=$DISTRIB_ID
        version_id=$DISTRIB_RELEASE
    else
        os=$(grep -Eo 'centos|debian|ubuntu' /etc/issue 2>/dev/null || 
             grep -Eo 'centos|debian|ubuntu' /proc/version 2>/dev/null || 
             echo "unknown")
        version_id=$(grep -Eo '[0-9]+' /etc/issue 2>/dev/null || echo "0")
    fi

    os=$(echo "$os" | tr '[:upper:]' '[:lower:]')

    # Version validation
    local min_version
    case "$os" in
        centos)  min_version=7 ;;
        ubuntu)  min_version=16 ;;
        debian)  min_version=8 ;;
        *)
            log_error "Unsupported operating system: $os"
            exit 1
            ;;
    esac

    if [[ $(echo "$version_id" | cut -d. -f1) -lt $min_version ]]; then
        log_error "Please use ${os^} $min_version or a higher version"
        exit 1
    fi

    echo "$os"
}

install_dependencies() {
    local os=$1
    log_info "Installing dependencies..."
    
    # Array of required packages
    local packages=("wget" "curl" "tar")
    
    if [[ "$os" == "centos" ]]; then
        yum install epel-release -y
        yum install "${packages[@]}" -y
    else
        apt-get update -y
        apt-get install "${packages[@]}" -y
    fi

    # Verify installations
    local failed_packages=()
    for pkg in "${packages[@]}"; do
        if ! command -v "$pkg" >/dev/null 2>&1; then
            failed_packages+=("$pkg")
        fi
    done

    if [[ ${#failed_packages[@]} -ne 0 ]]; then
        log_error "Failed to install packages: ${failed_packages[*]}"
        exit 1
    fi
}

get_latest_version() {
    if [[ -z "$version" ]]; then
        local api_response
        api_response=$(curl -sL "$GITHUB_API_URL")
        if [[ $? -ne 0 ]]; then
            log_error "Failed to fetch latest version from GitHub API"
            exit 1
        fi
        echo "$api_response" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        echo "${version#v}"
    fi
}

download_and_install() {
    log_info "Downloading VMonitor ${version}..."

    local download_url="${GITHUB_DOWNLOAD_URL}/${version}/vmonitor_${arch}.tar.gz"
    local archive_path="${TEMP_DIR}/vmonitor.tar.gz"

    # Download with progress bar
    wget --progress=bar:force -O "$archive_path" "$download_url" 2>&1

    if [[ ! -f "$archive_path" ]]; then
        log_error "Download failed"
        exit 1
    fi

    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"

    # Extract archive
    if ! tar -xzf "$archive_path" -C "$TEMP_DIR"; then
        log_error "Failed to extract archive"
        exit 1
    fi

    # Install binary
    install -m 755 "${TEMP_DIR}/vmonitor" "$BIN_PATH"

    # Download and install service file
    if ! wget -q -O "$SERVICE_PATH" "$SERVICE_FILE_URL"; then
        log_error "Failed to download service file"
        exit 1
    fi

    systemctl daemon-reload
}

setup_config() {
    log_info "Setting up configuration..."

    # Interactive prompt if parameters not provided
    if [[ -z "$server" ]]; then
        read -p "Enter server address (e.g., ws://localhost:3000/wss/monitor): " server
    fi
    if [[ -z "$secret" ]]; then
        read -s -p "Enter secret token: " secret
        echo
    fi

    # Validate URL format
    if ! echo "$server" | grep -qE '^(ws|wss)://'; then
        log_error "Invalid WebSocket URL format. Must start with ws:// or wss://"
        exit 1
    fi

    # Create config with proper permissions
    cat > "$CONFIG_PATH" << EOF
# Default connection settings
[connection]
base_delay = 1
max_delay = 60
max_retries = -1

# Endpoint configuration
[[endpoints]]
name = "default"
server = "${server}"
secret = "${secret}"
enabled = true
EOF

    chmod 600 "$CONFIG_PATH"
    log_info "Configuration saved to ${CONFIG_PATH}"
}

setup_service() {
    log_info "Setting up systemd service..."

    # Stop service if running
    if systemctl is-active vmonitor >/dev/null 2>&1; then
        systemctl stop vmonitor
    fi

    systemctl enable vmonitor
    systemctl start vmonitor
    
    # Wait and verify service status
    local max_attempts=5
    local attempt=1
    
    while [[ $attempt -le $max_attempts ]]; do
        if systemctl is-active vmonitor >/dev/null 2>&1; then
            log_info "VMonitor service started successfully"
            return 0
        fi
        sleep 2
        ((attempt++))
    done

    log_error "VMonitor service failed to start. Check logs with: journalctl -u vmonitor"
    exit 1
}

main() {
    log_info "Starting VMonitor installation..."
    
    check_root
    
    local os
    os=$(detect_os)
    
    if [[ -z "$arch" ]]; then
        arch=$(detect_architecture)
    fi
    
    install_dependencies "$os"
    version=$(get_latest_version)
    
    download_and_install
    setup_config
    setup_service
    
    log_info "Installation completed successfully"
}

# Parse command line arguments
server=$1
secret=$2
version=""
arch=""
shift 2

while [[ $# -gt 0 ]]; do
    case "$1" in
        --server)
            server="$2"
            shift 2
            ;;
        --secret)
            secret="$2"
            shift 2
            ;;
        --version)
            version="$2"
            shift 2
            ;;
        --arch)
            arch="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [URL] [TOKEN] [--version VERSION] [--arch ARCH]"
            exit 0
            ;;
        *)
            log_error "Unknown parameter: $1"
            exit 1
            ;;
    esac
done

main "$@"