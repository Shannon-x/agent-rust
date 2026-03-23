#!/bin/bash
# 哪吒监控 Agent (Rust) 一键管理脚本
# 安装: curl -sL https://raw.githubusercontent.com/Shannon-x/agent-rust/main/install.sh | sudo bash
# 指定操作: curl -sL ... | sudo bash -s -- install --server <地址> --secret <密钥>

set -euo pipefail

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

log()  { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
err()  { echo -e "${RED}[✗]${NC} $*" >&2; }

# ─── 架构检测 ────────────────────────────────────────────
detect_arch() {
    local arch
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

# ─── 获取最新版本 ─────────────────────────────────────────
get_latest_version() {
    curl -sL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | head -1 | cut -d'"' -f4
}

# ─── 下载二进制 ───────────────────────────────────────────
download_binary() {
    local version="$1"
    local arch="$2"
    local url="https://github.com/${REPO}/releases/download/${version}/${AGENT_NAME}_${arch}.zip"
    local tmp="/tmp/${AGENT_NAME}.zip"

    log "下载 ${AGENT_NAME} ${version} (${arch})..."
    curl -sL "${url}" -o "${tmp}"

    if [[ ! -s "${tmp}" ]]; then
        err "下载失败: ${url}"
        exit 1
    fi

    mkdir -p "${INSTALL_DIR}"
    unzip -o "${tmp}" -d "${INSTALL_DIR}" >/dev/null 2>&1
    chmod +x "${INSTALL_DIR}/${AGENT_NAME}"
    rm -f "${tmp}"
    log "二进制已安装到 ${INSTALL_DIR}/${AGENT_NAME}"
}

# ─── 安装 ─────────────────────────────────────────────────
do_install() {
    local server="" secret="" tls="false" gpu="false" temp="false" debug="false"

    # 解析参数
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --server|-s)       server="$2"; shift 2 ;;
            --secret|-k)       secret="$2"; shift 2 ;;
            --tls)             tls="true"; shift ;;
            --gpu)             gpu="true"; shift ;;
            --temperature)     temp="true"; shift ;;
            --debug)           debug="true"; shift ;;
            *)                 shift ;;
        esac
    done

    # 交互输入
    if [[ -z "$server" ]]; then
        read -rp "$(echo -e "${CYAN}请输入面板 gRPC 地址 [如 panel.example.com:8008]: ${NC}")" server
        [[ -z "$server" ]] && { err "面板地址不能为空"; exit 1; }
    fi
    if [[ -z "$secret" ]]; then
        read -rp "$(echo -e "${CYAN}请输入客户端密钥 (Client Secret): ${NC}")" secret
        [[ -z "$secret" ]] && { err "客户端密钥不能为空"; exit 1; }
    fi

    # 检查是否已安装
    if systemctl is-active --quiet "${SERVICE_NAME}" 2>/dev/null; then
        warn "检测到已运行的 Agent，将停止后重新安装..."
        systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
    fi

    # 下载
    local arch version
    arch=$(detect_arch)
    version=$(get_latest_version)
    if [[ -z "$version" ]]; then
        err "无法获取最新版本，请检查网络或访问 https://github.com/${REPO}/releases"
        exit 1
    fi
    download_binary "$version" "$arch"

    # 生成 UUID
    local uuid
    uuid=$(cat /proc/sys/kernel/random/uuid 2>/dev/null || uuidgen 2>/dev/null || python3 -c "import uuid; print(uuid.uuid4())" 2>/dev/null || echo "$(head -c 16 /dev/urandom | xxd -p)")

    # 写入配置
    cat > "${CONFIG_FILE}" <<EOF
# 哪吒监控 Agent 配置 - $(date '+%Y-%m-%d %H:%M:%S')
server: "${server}"
client_secret: "${secret}"
uuid: "${uuid}"
tls: ${tls}
debug: ${debug}
report_delay: 3
gpu: ${gpu}
temperature: ${temp}
skip_connection_count: false
ip_report_period: 1800
EOF

    log "配置已生成: ${CONFIG_FILE}"

    # 创建 systemd 服务
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

    sleep 2
    if systemctl is-active --quiet "${SERVICE_NAME}"; then
        echo ""
        echo -e "${GREEN}╔═══════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║         ✅ 安装成功!                          ║${NC}"
        echo -e "${GREEN}╚═══════════════════════════════════════════════╝${NC}"
        echo ""
        echo -e "  ${CYAN}版本:${NC}     ${version}"
        echo -e "  ${CYAN}架构:${NC}     ${arch}"
        echo -e "  ${CYAN}安装目录:${NC} ${INSTALL_DIR}"
        echo -e "  ${CYAN}配置文件:${NC} ${CONFIG_FILE}"
        echo -e "  ${CYAN}UUID:${NC}     ${uuid}"
        echo ""
        show_management_commands
    else
        err "服务启动失败"
        err "查看日志: journalctl -u ${SERVICE_NAME} -n 50 --no-pager"
        exit 1
    fi
}

# ─── 更新 ─────────────────────────────────────────────────
do_update() {
    if [[ ! -f "${INSTALL_DIR}/${AGENT_NAME}" ]]; then
        err "未检测到已安装的 Agent，请先安装"
        exit 1
    fi

    local arch version current_version
    arch=$(detect_arch)
    version=$(get_latest_version)

    if [[ -z "$version" ]]; then
        err "无法获取最新版本"
        exit 1
    fi

    current_version=$("${INSTALL_DIR}/${AGENT_NAME}" --version 2>/dev/null | awk '{print $2}' || echo "unknown")
    log "当前版本: ${current_version}"
    log "最新版本: ${version}"

    if [[ "v${current_version}" == "${version}" ]]; then
        log "已是最新版本，无需更新"
        return
    fi

    log "停止服务..."
    systemctl stop "${SERVICE_NAME}" 2>/dev/null || true

    download_binary "$version" "$arch"

    systemctl start "${SERVICE_NAME}"
    sleep 1

    if systemctl is-active --quiet "${SERVICE_NAME}"; then
        echo ""
        echo -e "${GREEN}╔═══════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║         ✅ 更新成功!                          ║${NC}"
        echo -e "${GREEN}╚═══════════════════════════════════════════════╝${NC}"
        echo ""
        echo -e "  ${CYAN}版本:${NC} ${current_version} → ${version}"
        echo ""
    else
        err "更新后服务启动失败"
        err "查看日志: journalctl -u ${SERVICE_NAME} -n 50 --no-pager"
        exit 1
    fi
}

# ─── 卸载 ─────────────────────────────────────────────────
do_uninstall() {
    echo -e "${YELLOW}即将卸载 Nezha Agent，此操作将:${NC}"
    echo "  - 停止并删除 systemd 服务"
    echo "  - 删除 ${INSTALL_DIR} (包括配置和二进制)"
    echo ""
    read -rp "$(echo -e "${RED}确认卸载? [y/N]: ${NC}")" confirm
    if [[ "${confirm,,}" != "y" ]]; then
        log "已取消卸载"
        return
    fi

    log "停止服务..."
    systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
    systemctl disable "${SERVICE_NAME}" 2>/dev/null || true

    log "删除服务文件..."
    rm -f "/etc/systemd/system/${SERVICE_NAME}.service"
    systemctl daemon-reload

    log "删除安装目录..."
    rm -rf "${INSTALL_DIR}"

    echo ""
    echo -e "${GREEN}╔═══════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║         ✅ 卸载完成!                          ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════╝${NC}"
    echo ""
}

# ─── 管理命令提示 ──────────────────────────────────────────
show_management_commands() {
    echo -e "  ${YELLOW}管理命令:${NC}"
    echo "    systemctl status ${SERVICE_NAME}     # 查看状态"
    echo "    systemctl restart ${SERVICE_NAME}    # 重启"
    echo "    systemctl stop ${SERVICE_NAME}       # 停止"
    echo "    journalctl -u ${SERVICE_NAME} -f     # 查看日志"
    echo ""
    echo -e "  ${YELLOW}脚本命令:${NC}"
    echo "    sudo bash install.sh update          # 更新到最新版"
    echo "    sudo bash install.sh uninstall       # 卸载"
    echo ""
}

# ─── 交互菜单 ─────────────────────────────────────────────
show_menu() {
    echo ""
    echo -e "${CYAN}╔═══════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║   ${BOLD}哪吒监控 Agent (Rust 高性能版) 管理脚本${NC}${CYAN}    ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════╝${NC}"
    echo ""

    local installed="false"
    if [[ -f "${INSTALL_DIR}/${AGENT_NAME}" ]]; then
        installed="true"
        local ver
        ver=$("${INSTALL_DIR}/${AGENT_NAME}" --version 2>/dev/null | awk '{print $2}' || echo "?")
        local status
        status=$(systemctl is-active "${SERVICE_NAME}" 2>/dev/null || echo "未运行")
        echo -e "  当前状态: ${GREEN}已安装${NC} (v${ver}, ${status})"
    else
        echo -e "  当前状态: ${YELLOW}未安装${NC}"
    fi
    echo ""
    echo -e "  ${BOLD}1)${NC} 安装 / 重新安装"
    echo -e "  ${BOLD}2)${NC} 更新到最新版"
    echo -e "  ${BOLD}3)${NC} 卸载"
    echo -e "  ${BOLD}0)${NC} 退出"
    echo ""
    read -rp "$(echo -e "${CYAN}请选择操作 [0-3]: ${NC}")" choice

    case "$choice" in
        1) do_install ;;
        2) do_update ;;
        3) do_uninstall ;;
        0) echo "退出"; exit 0 ;;
        *) err "无效选择"; exit 1 ;;
    esac
}

# ─── 入口 ─────────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
    err "请使用 root 权限运行"
    err "用法: curl -sL https://raw.githubusercontent.com/${REPO}/main/install.sh | sudo bash"
    exit 1
fi

ACTION="${1:-}"
shift 2>/dev/null || true

case "$ACTION" in
    install)    do_install "$@" ;;
    update)     do_update ;;
    uninstall)  do_uninstall ;;
    "")         show_menu ;;
    *)          err "未知操作: $ACTION"; echo "用法: $0 {install|update|uninstall}"; exit 1 ;;
esac
