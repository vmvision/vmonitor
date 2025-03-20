#!/bin/bash

readonly GITHUB_BASE_URL="https://github.com/vmvision/vmonitor"
readonly GITHUB_API_URL="https://api.github.com/repos/vmvision/vmonitor/releases/latest"
readonly GITHUB_DOWNLOAD_URL="${GITHUB_BASE_URL}/releases/download"
readonly SERVICE_FILE_URL="https://raw.githubusercontent.com/vmvision/vmonitor/refs/heads/master/release/vmonitor.service"
readonly INSTALL_DIR="/etc/vmonitor"
readonly BIN_PATH="/usr/local/bin/vmonitor"
readonly SERVICE_PATH="/etc/systemd/system/vmonitor.service"
declare -A colors=(
    ["red"]='\033[0;31m'
    ["green"]='\033[0;32m'
    ["yellow"]='\033[0;33m'
    ["plain"]='\033[0m'
)
log_info() {
    echo -e "${colors[green]}[INFO]${colors[plain]} $1"
}
log_warn() {
    echo -e "${colors[yellow]}[WARN]${colors[plain]} $1"
}
log_error() {
    echo -e "${colors[red]}[ERROR]${colors[plain]} $1"
}
check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "You must use the root user to run this script！"
        exit 1
    fi
}
detect_architecture() {
    local detected_arch=$(arch)
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
            log_warn "Failure to detect architecture, use default architecture: x86_64-unknown-linux-gnu"
            echo "x86_64-unknown-linux-gnu"
            ;;
    esac
}
detect_os() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        OS=$ID
        VERSION_ID=$VERSION_ID
    elif [[ -f /etc/lsb-release ]]; then
        . /etc/lsb-release
        OS=$DISTRIB_ID
        VERSION_ID=$DISTRIB_RELEASE
    else
        OS=$(grep -Eo 'centos|debian|ubuntu' /etc/issue 2>/dev/null || grep -Eo 'centos|debian|ubuntu' /proc/version 2>/dev/null || echo "unknown")
        VERSION_ID=$(grep -Eo '[0-9]+' /etc/issue 2>/dev/null || echo "0")
    fi
    OS=$(echo "$OS" | tr '[:upper:]' '[:lower:]')
    case "$OS" in
        centos)
            if [[ $(echo "$VERSION_ID" | cut -d. -f1) -le 6 ]]; then
                log_error "Please use CentOS 7 or a higher version of the system!" && exit 1
            fi
            ;;
        ubuntu)
            if [[ $(echo "$VERSION_ID" | cut -d. -f1) -lt 16 ]]; then
                log_error "Please use Ubuntu 16 or a higher version of the system!" && exit 1
            fi
            ;;
        debian)
            if [[ $(echo "$VERSION_ID" | cut -d. -f1) -lt 8 ]]; then
                log_error "Please use Debian 8 or a higher version of the system!" && exit 1
            fi
            ;;
        *)
            log_error "No supported system version detected！" && exit 1
            ;;
    esac
    echo "$OS"
}
install_dependencies() {
    local os=$1
    log_info "Installing dependencies..."
    
    if [[ "$os" == "centos" ]]; then
        yum install epel-release -y
        yum install wget -y
    else
        apt update -y
        apt install wget -y
    fi
}
get_latest_version() {
    if [[ -z "$version" ]]; then
        curl -Ls "$GITHUB_API_URL" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        echo "${version#v}"
    fi
}
download_and_install() {
    log_info "Downloading VMonitor ${version}..."

    local download_url="${GITHUB_DOWNLOAD_URL}/${version}/vmonitor_${arch}.tar.gz"
    wget -q -N --no-check-certificate -O "${INSTALL_DIR}/vmonitor.tar.gz" "$download_url"
    tar -xzf "${INSTALL_DIR}/vmonitor.tar.gz" -C "${INSTALL_DIR}"
    if [[ $? -ne 0 ]]; then
        log_error "Download failed, please check the network connection and version number!"
        exit 1
    fi
    chmod +x ${INSTALL_DIR}/vmonitor
    mv ${INSTALL_DIR}/vmonitor "$BIN_PATH"
    wget -q -N --no-check-certificate -O "$SERVICE_PATH" "$SERVICE_FILE_URL"
    systemctl daemon-reload
}
setup_config() {
    log_info "Setting up configuration..."
    if [[ -z "$url" ]]; then
        read -p "Please enter server address: " url
    fi
    if [[ -z "$token" ]]; then
        read -p "Please enter secret token: " token
    fi
    cat > /etc/vmonitor/config.yaml << EOF
websocket_url = "${url}"
auth_secret= "${token}"
interval = 3

[connection]
base_delay = 1
max_delay = 60
max_retries = -1
EOF
    chmod 600 /etc/vmonitor/config.yaml
    log_info "Configuration saved to /etc/vmonitor/config.yaml"
}
setup_service() {
    systemctl stop vmonitor 2>/dev/null
    systemctl enable vmonitor
    systemctl start vmonitor
    
    sleep 2
    if systemctl is-active vmonitor >/dev/null 2>&1; then
        log_info "VMonitor service started successfully！"
    else
        log_warn "VMonitor service may have failed to start, please check the logs"
    fi
}
main() {
    log_info "Start Installation VMonitor..."
    # Prepare
    check_root
    local os=$(detect_os)
    if [[ -z "$arch" ]]; then
        arch=$(detect_architecture)
    fi
    install_dependencies "$os"

    mkdir -p "$INSTALL_DIR"
    version=$(get_latest_version "$version")
    download_and_install "$version" "$arch"
    setup_config "$url" "$token"
    setup_service
}
url=$1
token=$2
version=""
arch=""
shift 2
while [[ $# -gt 0 ]]; do
    case "$1" in
        --url)
            url="$2"
            shift 2
            ;;
        --token)
            token="$2"
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
        *)
            log_error "Unknown parameter: $1"
            exit 1
            ;;
    esac
done
main "$@"