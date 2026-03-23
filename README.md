# 哪吒监控 Agent (Rust 高性能版)

[![CI](https://github.com/Shannon-x/agent-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/Shannon-x/agent-rust/actions)

使用 Rust 重写的哪吒监控 Agent，实现高性能、低资源占用。

## 特性

- 🚀 **高性能**: 直接读取 `/proc`、`/sys`，零 GC 开销
- 📦 **极小体积**: Release 二进制仅 ~4MB (stripped + LTO)
- 🔌 **完全兼容**: 与哪吒面板 gRPC 协议完全兼容
- 🖥️ **系统监控**: CPU、内存、磁盘、网络、负载、温度、GPU、连接数
- 📡 **GeoIP 上报**: 支持 IPv4/IPv6 双栈 IP 获取与国家码上报
- ⚡ **任务执行**: HTTP GET、ICMP/TCP Ping、命令执行、配置管理
- 🔧 **服务管理**: systemd 服务安装/卸载/启动/停止/重启

## 构建

```bash
# 安装依赖
apt install protobuf-compiler

# 构建 Release
cargo build --release

# 生成的二进制在 target/release/nezha-agent
```

## 使用

```bash
# 运行代理
./nezha-agent -c config.yml

# 查看帮助
./nezha-agent --help

# 安装为 systemd 服务
./nezha-agent service install -c /etc/nezha/config.yml

# 管理服务
./nezha-agent service start|stop|restart|uninstall
```

## 配置文件

```yaml
server: "dashboard.example.com:8008"
client_secret: "your-secret"
tls: false
debug: false
report_delay: 3
temperature: true
gpu: false
```

也支持 `NZ_` 前缀环境变量覆盖，例如：
```bash
NZ_SERVER="dashboard.example.com:8008" NZ_CLIENT_SECRET="secret" ./nezha-agent
```

## 交叉编译

支持的目标平台：
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

```bash
# 使用 cross 工具
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu
```

## License

Apache-2.0
