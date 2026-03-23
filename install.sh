#!/bin/sh
# 哪吒监控 Agent (Rust) 一键管理脚本
# 安装: curl -sL https://raw.githubusercontent.com/Shannon-x/agent-rust/main/install.sh | sudo sh
# 指定操作: curl -sL ... | sudo sh -s -- install --server <地址> --secret <密钥>

set -u

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

AGENT_NAME="nezha-agent"
INSTALL_DIR="/opt/nezha-agent"
CONFIG_FILE="${INSTALL_DIR}/config.yml"
SERVICE_NAME="nezha-agent"
REPO="Shannon-x/agent-rust"

log()  { printf "${GREEN}[✓]${NC} %s\n" "$*"; }
warn() { printf "${YELLOW}[!]${NC} %s\n" "$*"; }
err()  { printf "${RED}[✗]${NC} %s\n" "$*" >&2; }

# 从 /dev/tty 读取用户输入 (兼容 curl|sh 管道模式)
ask() {
    printf "%s" "$1"
    read -r REPLY </dev/tty || { err "无法读取输入 (请使用: sudo sh -c \"\$(curl -sL URL)\")"; exit 1; }
}

detect_arch() {
    arch=$(uname -m)
    case "$arch" in
        x86_64|amd64)    echo "linux_amd64" ;;
        aarch64|arm64)   echo "linux_arm64" ;;
        armv7*|armhf)    echo "linux_arm" ;;
        i386|i686)       echo "linux_386" ;;
        riscv64)         echo "linux_riscv64" ;;
        s390x)           echo "linux_s390x" ;;
        *)               err "不支持的架构: $arch"; exit 1 ;;
    esac
}

get_latest_version() {
    curl -sL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | head -1 | cut -d'"' -f4
}

download_binary() {
    _version="$1"
    _arch="$2"
    _url="https://github.com/${REPO}/releases/download/${_version}/${AGENT_NAME}_${_arch}.zip"
    _tmp="/tmp/${AGENT_NAME}.zip"

    log "下载 ${AGENT_NAME} ${_version} (${_arch})..."
    curl -sL "${_url}" -o "${_tmp}"

    if [ ! -s "${_tmp}" ]; then
        err "下载失败: ${_url}"
        exit 1
    fi

    mkdir -p "${INSTALL_DIR}"

    if command -v unzip >/dev/null 2>&1; then
        unzip -o "${_tmp}" -d "${INSTALL_DIR}" >/dev/null 2>&1
    elif command -v busybox >/dev/null 2>&1; then
        busybox unzip -o "${_tmp}" -d "${INSTALL_DIR}" 2>/dev/null
    elif command -v python3 >/dev/null 2>&1; then
        python3 -c "import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "${_tmp}" "${INSTALL_DIR}"
    else
        err "未找到 unzip，请安装: apt install unzip 或 apk add unzip"
        rm -f "${_tmp}"
        exit 1
    fi

    chmod +x "${INSTALL_DIR}/${AGENT_NAME}"
    rm -f "${_tmp}"
    log "二进制已安装到 ${INSTALL_DIR}/${AGENT_NAME}"
}

gen_uuid() {
    if [ -f /proc/sys/kernel/random/uuid ]; then
        cat /proc/sys/kernel/random/uuid
    elif command -v uuidgen >/dev/null 2>&1; then
        uuidgen
    elif command -v python3 >/dev/null 2>&1; then
        python3 -c "import uuid; print(uuid.uuid4())"
    else
        head -c 16 /dev/urandom 2>/dev/null | od -A n -t x1 | tr -d ' \n' | sed 's/\(.\{8\}\)\(.\{4\}\)\(.\{4\}\)\(.\{4\}\)\(.\{12\}\)/\1-\2-\3-\4-\5/'
    fi
}

install_service() {
    if command -v systemctl >/dev/null 2>&1; then
        cat > "/etc/systemd/system/${SERVICE_NAME}.service" <<EOF
[Unit]
Description=Nezha Agent (Rust)
After=network.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/${AGENT_NAME} -c ${CONFIG_FILE}
Restart=always
RestartSec=5
LimitNOFILE=65535
WorkingDirectory=${INSTALL_DIR}

[Install]
WantedBy=multi-user.target
EOF
        systemctl daemon-reload
        systemctl enable "${SERVICE_NAME}" >/dev/null 2>&1
        systemctl start "${SERVICE_NAME}"
    elif command -v rc-service >/dev/null 2>&1; then
        cat > "/etc/init.d/${SERVICE_NAME}" <<EOF
#!/sbin/openrc-run
name="${SERVICE_NAME}"
description="Nezha Agent (Rust)"
command="${INSTALL_DIR}/${AGENT_NAME}"
command_args="-c ${CONFIG_FILE}"
command_background=true
pidfile="/run/${SERVICE_NAME}.pid"
output_log="/var/log/${SERVICE_NAME}.log"
error_log="/var/log/${SERVICE_NAME}.err"
EOF
        chmod +x "/etc/init.d/${SERVICE_NAME}"
        rc-update add "${SERVICE_NAME}" default 2>/dev/null
        rc-service "${SERVICE_NAME}" start 2>/dev/null
    else
        warn "未检测到 systemd 或 openrc，请手动启动:"
        warn "  ${INSTALL_DIR}/${AGENT_NAME} -c ${CONFIG_FILE}"
    fi
}

stop_service() {
    if command -v systemctl >/dev/null 2>&1; then
        systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
    elif command -v rc-service >/dev/null 2>&1; then
        rc-service "${SERVICE_NAME}" stop 2>/dev/null || true
    fi
}

is_service_active() {
    if command -v systemctl >/dev/null 2>&1; then
        systemctl is-active --quiet "${SERVICE_NAME}" 2>/dev/null
    elif command -v rc-service >/dev/null 2>&1; then
        rc-service "${SERVICE_NAME}" status >/dev/null 2>&1
    else
        return 1
    fi
}

# ─── 安装 ─────────────────────────────────────────────────
do_install() {
    _server="" _secret="" _tls="false" _gpu="false" _temp="false" _debug="false"

    while [ $# -gt 0 ]; do
        case "$1" in
            --server|-s)       _server="$2"; shift 2 ;;
            --secret|-k)       _secret="$2"; shift 2 ;;
            --tls)             _tls="true"; shift ;;
            --gpu)             _gpu="true"; shift ;;
            --temperature)     _temp="true"; shift ;;
            --debug)           _debug="true"; shift ;;
            *)                 shift ;;
        esac
    done

    if [ -z "$_server" ]; then
        ask "$(printf "${CYAN}请输入面板 gRPC 地址 [如 panel.example.com:8008]: ${NC}")"
        _server="$REPLY"
        [ -z "$_server" ] && { err "面板地址不能为空"; exit 1; }
    fi
    if [ -z "$_secret" ]; then
        ask "$(printf "${CYAN}请输入客户端密钥 (Client Secret): ${NC}")"
        _secret="$REPLY"
        [ -z "$_secret" ] && { err "客户端密钥不能为空"; exit 1; }
    fi

    if is_service_active; then
        warn "检测到已运行的 Agent，将停止后重新安装..."
        stop_service
    fi

    _arch=$(detect_arch)
    _version=$(get_latest_version)
    if [ -z "$_version" ]; then
        err "无法获取最新版本，请检查网络"
        exit 1
    fi
    download_binary "$_version" "$_arch"

    _uuid=$(gen_uuid)

    cat > "${CONFIG_FILE}" <<EOF
# 哪吒监控 Agent 配置 - $(date '+%Y-%m-%d %H:%M:%S')
server: "${_server}"
client_secret: "${_secret}"
uuid: "${_uuid}"
tls: ${_tls}
debug: ${_debug}
report_delay: 3
gpu: ${_gpu}
temperature: ${_temp}
skip_connection_count: false
ip_report_period: 1800
EOF

    log "配置已生成: ${CONFIG_FILE}"
    install_service

    sleep 2
    if is_service_active; then
        printf "\n"
        printf "${GREEN}╔═══════════════════════════════════════════════╗${NC}\n"
        printf "${GREEN}║         ✅ 安装成功!                          ║${NC}\n"
        printf "${GREEN}╚═══════════════════════════════════════════════╝${NC}\n"
        printf "\n"
        printf "  ${CYAN}版本:${NC}     %s\n" "$_version"
        printf "  ${CYAN}架构:${NC}     %s\n" "$_arch"
        printf "  ${CYAN}安装目录:${NC} %s\n" "$INSTALL_DIR"
        printf "  ${CYAN}配置文件:${NC} %s\n" "$CONFIG_FILE"
        printf "  ${CYAN}UUID:${NC}     %s\n" "$_uuid"
        printf "\n"
        show_management_commands
    else
        err "服务启动失败，请检查日志"
        exit 1
    fi
}

# ─── 更新 ─────────────────────────────────────────────────
do_update() {
    if [ ! -f "${INSTALL_DIR}/${AGENT_NAME}" ]; then
        err "未检测到已安装的 Agent，请先安装"
        exit 1
    fi

    _arch=$(detect_arch)
    _version=$(get_latest_version)
    [ -z "$_version" ] && { err "无法获取最新版本"; exit 1; }

    _cur=$("${INSTALL_DIR}/${AGENT_NAME}" --version 2>/dev/null | awk '{print $2}') || _cur="unknown"
    log "当前版本: ${_cur}"
    log "最新版本: ${_version}"

    if [ "v${_cur}" = "${_version}" ]; then
        log "已是最新版本，无需更新"
        return
    fi

    log "停止服务..."
    stop_service
    download_binary "$_version" "$_arch"

    if command -v systemctl >/dev/null 2>&1; then
        systemctl start "${SERVICE_NAME}"
    elif command -v rc-service >/dev/null 2>&1; then
        rc-service "${SERVICE_NAME}" start
    fi

    sleep 1
    if is_service_active; then
        printf "\n"
        printf "${GREEN}╔═══════════════════════════════════════════════╗${NC}\n"
        printf "${GREEN}║         ✅ 更新成功!                          ║${NC}\n"
        printf "${GREEN}╚═══════════════════════════════════════════════╝${NC}\n"
        printf "\n  ${CYAN}版本:${NC} %s → %s\n\n" "$_cur" "$_version"
    else
        err "更新后服务启动失败"
        exit 1
    fi
}

# ─── 卸载 ─────────────────────────────────────────────────
do_uninstall() {
    printf "${YELLOW}即将卸载 Nezha Agent:${NC}\n"
    printf "  - 停止并删除服务\n"
    printf "  - 删除 %s\n\n" "$INSTALL_DIR"
    ask "$(printf "${RED}确认卸载? [y/N]: ${NC}")"
    case "$REPLY" in
        y|Y) ;;
        *)   log "已取消"; return ;;
    esac

    stop_service

    if command -v systemctl >/dev/null 2>&1; then
        systemctl disable "${SERVICE_NAME}" 2>/dev/null || true
        rm -f "/etc/systemd/system/${SERVICE_NAME}.service"
        systemctl daemon-reload
    elif command -v rc-service >/dev/null 2>&1; then
        rc-update del "${SERVICE_NAME}" default 2>/dev/null || true
        rm -f "/etc/init.d/${SERVICE_NAME}"
    fi

    rm -rf "${INSTALL_DIR}"

    printf "\n"
    printf "${GREEN}╔═══════════════════════════════════════════════╗${NC}\n"
    printf "${GREEN}║         ✅ 卸载完成!                          ║${NC}\n"
    printf "${GREEN}╚═══════════════════════════════════════════════╝${NC}\n\n"
}

show_management_commands() {
    if command -v systemctl >/dev/null 2>&1; then
        printf "  ${YELLOW}管理命令:${NC}\n"
        printf "    systemctl status %s     # 查看状态\n" "$SERVICE_NAME"
        printf "    systemctl restart %s    # 重启\n" "$SERVICE_NAME"
        printf "    systemctl stop %s       # 停止\n" "$SERVICE_NAME"
        printf "    journalctl -u %s -f     # 查看日志\n\n" "$SERVICE_NAME"
    fi
}

# ─── 交互菜单 ─────────────────────────────────────────────
show_menu() {
    printf "\n"
    printf "${CYAN}╔═══════════════════════════════════════════════╗${NC}\n"
    printf "${CYAN}║   ${BOLD}哪吒监控 Agent (Rust) 管理脚本${NC}${CYAN}              ║${NC}\n"
    printf "${CYAN}╚═══════════════════════════════════════════════╝${NC}\n\n"

    if [ -f "${INSTALL_DIR}/${AGENT_NAME}" ]; then
        _ver=$("${INSTALL_DIR}/${AGENT_NAME}" --version 2>/dev/null | awk '{print $2}') || _ver="?"
        if is_service_active; then
            _st="运行中"
        else
            _st="已停止"
        fi
        printf "  当前状态: ${GREEN}已安装${NC} (v%s, %s)\n" "$_ver" "$_st"
    else
        printf "  当前状态: ${YELLOW}未安装${NC}\n"
    fi

    printf "\n"
    printf "  ${BOLD}1)${NC} 安装 / 重新安装\n"
    printf "  ${BOLD}2)${NC} 更新到最新版\n"
    printf "  ${BOLD}3)${NC} 卸载\n"
    printf "  ${BOLD}0)${NC} 退出\n\n"
    ask "$(printf "${CYAN}请选择操作 [0-3]: ${NC}")"

    case "$REPLY" in
        1) do_install ;;
        2) do_update ;;
        3) do_uninstall ;;
        0) printf "退出\n"; exit 0 ;;
        *) err "无效选择"; exit 1 ;;
    esac
}

# ─── 入口 ─────────────────────────────────────────────────
if [ "$(id -u)" -ne 0 ]; then
    err "请使用 root 权限运行"
    printf "  用法: curl -sL https://raw.githubusercontent.com/%s/main/install.sh | sudo sh\n" "$REPO"
    exit 1
fi

ACTION="${1:-}"
shift 2>/dev/null || true

case "$ACTION" in
    install)    do_install "$@" ;;
    update)     do_update ;;
    uninstall)  do_uninstall ;;
    "")         show_menu ;;
    *)          err "未知操作: $ACTION"; printf "用法: install|update|uninstall\n"; exit 1 ;;
esac
