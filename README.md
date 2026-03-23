# 哪吒监控 Agent (Rust 高性能版)

[![CI](https://github.com/Shannon-x/agent-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/Shannon-x/agent-rust/actions)

使用 Rust 重写的哪吒监控 Agent，实现高性能、低资源占用。

## 特性

- 🚀 **高性能**: 直接读取 `/proc`、`/sys`，零 GC 开销
- 📦 **极小体积**: Release 二进制仅 ~4MB (stripped + LTO)
- 🔌 **完全兼容**: 与哪吒面板 gRPC 协议完全兼容
- 🖥️ **系统监控**: CPU、内存、磁盘、网络、负载、温度、GPU、连接数
- ⚡ **任务执行**: HTTP GET、ICMP/TCP Ping、命令执行、配置管理

## 一键部署

### 交互式安装 (推荐)

```bash
curl -sL https://raw.githubusercontent.com/Shannon-x/agent-rust/main/install.sh | sudo bash
```

运行后会显示菜单，选择 **1) 安装**，按提示输入面板地址和密钥即可。UUID 自动生成。

### 命令行直接安装

```bash
curl -sL https://raw.githubusercontent.com/Shannon-x/agent-rust/main/install.sh | sudo bash -s -- install \
  --server panel.example.com:8008 \
  --secret YOUR_CLIENT_SECRET \
  --tls
```

### 更新到最新版

```bash
curl -sL https://raw.githubusercontent.com/Shannon-x/agent-rust/main/install.sh | sudo bash -s -- update
```

### 卸载

```bash
curl -sL https://raw.githubusercontent.com/Shannon-x/agent-rust/main/install.sh | sudo bash -s -- uninstall
```

### 安装参数

| 参数 | 说明 |
|------|------|
| `--server`, `-s` | 面板 gRPC 地址 (必填) |
| `--secret`, `-k` | 客户端密钥 (必填) |
| `--tls` | 启用 TLS 连接 |
| `--gpu` | 启用 GPU 监控 |
| `--temperature` | 启用温度监控 |
| `--debug` | 启用调试日志 |

## 服务管理

```bash
systemctl status nezha-agent     # 查看状态
systemctl restart nezha-agent    # 重启
systemctl stop nezha-agent       # 停止
journalctl -u nezha-agent -f     # 实时日志
```

## 手动构建

```bash
# 安装依赖
apt install protobuf-compiler

# 构建
cargo build --release
```

## 支持平台

| 平台 | 架构 |
|------|------|
| Linux | x86_64, i686, aarch64, armv7, riscv64, s390x |
| Linux (musl) | x86_64, aarch64 |

## License

Apache-2.0
