#!/bin/bash
# 哪吒监控 Agent (Rust) 一键部署脚本
# 用法: curl -sL https://raw.githubusercontent.com/Shannon-x/agent-rust/main/install.sh | bash -s -- --server <面板地址:端口> --secret <客户端密钥>

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

AGENT_NAME="nezha-agent"
INSTALL_DIR="/opt/nezha-agent"
CONFIG_FILE="${INSTALL_DIR}/config.yml"
SERVICE_NAME="nezha-agent"
REPO="Shannon-x/agent-rust"

log() { echo -e "${GREEN}[NEZHA]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
err() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# 检测架构
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

# 解析参数
SERVER=""
SECRET=""
TLS="false"
GPU="false"
TEMP="false"
DEBUG="false"
SKIP_CONN="false"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --server|-s)   SERVER="$2"; shift 2 ;;
        --secret|-k)   SECRET="$2"; shift 2 ;;
        --tls)         TLS="true"; shift ;;
        --gpu)         GPU="true"; shift ;;
        --temperature) TEMP="true"; shift ;;
        --debug)       DEBUG="true"; shift ;;
        --skip-conn)   SKIP_CONN="true"; shift ;;
        --help|-h)
            echo "用法: $0 --server <面板地址:端口> --secret <客户端密钥> [选项]"
            echo ""
            echo "必需参数:"
            echo "  --server, -s    面板 gRPC 地址 (如: panel.example.com:8008)"
            echo "  --secret, -k    客户端密钥 (从面板获取)"
            echo ""
            echo "可选参数:"
            echo "  --tls           启用 TLS 连接"
            echo "  --gpu           启用 GPU 监控"
            echo "  --temperature   启用温度监控"
            echo "  --debug         启用调试日志"
            echo "  --skip-conn     跳过连接数统计"
            exit 0
            ;;
        *)  err "未知参数: $1"; exit 1 ;;
    esac
done

# 交互模式 - 如果没有提供参数
if [[ -z "$SERVER" ]]; then
    echo -e "${CYAN}╔══════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║   哪吒监控 Agent (Rust) 一键部署         ║${NC}"
    echo -e "${CYAN}╚══════════════════════════════════════════╝${NC}"
    echo ""
    read -rp "$(echo -e ${YELLOW}请输入面板 gRPC 地址 [如 panel.example.com:8008]: ${NC})" SERVER
    if [[ -z "$SERVER" ]]; then
        err "面板地址不能为空"
        exit 1
    fi
fi

if [[ -z "$SECRET" ]]; then
    read -rp "$(echo -e ${YELLOW}请输入客户端密钥: ${NC})" SECRET
    if [[ -z "$SECRET" ]]; then
        err "客户端密钥不能为空"
        exit 1
    fi
fi

# 主安装流程
main() {
    log "检测系统架构..."
    local arch
    arch=$(detect_arch)
    log "架构: ${arch}"

    # 检测是否已安装
    if [[ -f "${INSTALL_DIR}/${AGENT_NAME}" ]]; then
        warn "检测到已安装的 Agent，将进行升级..."
        systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
    fi

    # 创建安装目录
    mkdir -p "${INSTALL_DIR}"

    # 获取最新版本
    log "获取最新版本信息..."
    local latest_tag
    latest_tag=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | head -1 | cut -d'"' -f4)

    if [[ -z "$latest_tag" ]]; then
        warn "无法获取最新 release，尝试从 main 分支构建下载..."
        # 尝试下载 artifact (需要 tag release)
        err "请先在 GitHub 创建一个 Release (打 tag: git tag v1.0.0 && git push --tags)"
        err "或者手动构建: cargo build --release && cp target/release/nezha-agent ${INSTALL_DIR}/"
        exit 1
    fi

    local download_url="https://github.com/${REPO}/releases/download/${latest_tag}/${AGENT_NAME}_${arch}.zip"
    log "下载 ${download_url} ..."

    local tmp_zip="/tmp/${AGENT_NAME}_${arch}.zip"
    curl -sL "${download_url}" -o "${tmp_zip}"

    if [[ ! -s "${tmp_zip}" ]]; then
        err "下载失败"
        exit 1
    fi

    # 解压
    log "安装到 ${INSTALL_DIR} ..."
    unzip -o "${tmp_zip}" -d "${INSTALL_DIR}" >/dev/null
    chmod +x "${INSTALL_DIR}/${AGENT_NAME}"
    rm -f "${tmp_zip}"

    # 生成 UUID
    local uuid
    uuid=$(cat /proc/sys/kernel/random/uuid 2>/dev/null || uuidgen 2>/dev/null || python3 -c "import uuid; print(uuid.uuid4())" 2>/dev/null)

    # 写入配置
    log "生成配置文件..."
    cat > "${CONFIG_FILE}" <<EOF
# 哪吒监控 Agent 配置 - 自动生成于 $(date)
server: "${SERVER}"
client_secret: "${SECRET}"
uuid: "${uuid}"
tls: ${TLS}
debug: ${DEBUG}
report_delay: 3
gpu: ${GPU}
temperature: ${TEMP}
skip_connection_count: ${SKIP_CONN}
ip_report_period: 1800
EOF

    log "配置文件: ${CONFIG_FILE}"
    log "UUID: ${uuid}"

    # 创建 systemd 服务
    log "创建 systemd 服务..."
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
    systemctl enable "${SERVICE_NAME}"
    systemctl start "${SERVICE_NAME}"

    # 验证
    sleep 2
    if systemctl is-active --quiet "${SERVICE_NAME}"; then
        echo ""
        echo -e "${GREEN}╔══════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║       ✅ 部署成功!                       ║${NC}"
        echo -e "${GREEN}╚══════════════════════════════════════════╝${NC}"
        echo ""
        echo -e "  ${CYAN}服务状态:${NC} $(systemctl is-active ${SERVICE_NAME})"
        echo -e "  ${CYAN}安装目录:${NC} ${INSTALL_DIR}"
        echo -e "  ${CYAN}配置文件:${NC} ${CONFIG_FILE}"
        echo -e "  ${CYAN}UUID:${NC}     ${uuid}"
        echo ""
        echo -e "  ${YELLOW}管理命令:${NC}"
        echo "    systemctl status ${SERVICE_NAME}    # 查看状态"
        echo "    systemctl restart ${SERVICE_NAME}   # 重启"
        echo "    systemctl stop ${SERVICE_NAME}      # 停止"
        echo "    journalctl -u ${SERVICE_NAME} -f    # 查看日志"
        echo ""
    else
        err "服务启动失败，请检查日志: journalctl -u ${SERVICE_NAME} -n 50"
        exit 1
    fi
}

# 需要 root
if [[ $EUID -ne 0 ]]; then
    err "请使用 root 权限运行此脚本"
    err "用法: sudo bash install.sh --server <地址> --secret <密钥>"
    exit 1
fi

main
