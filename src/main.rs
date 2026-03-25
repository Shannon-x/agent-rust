mod auth;
mod client;
mod config;
mod fm_proto;
mod ip;
mod monitor;
mod tasks;
mod util;

pub mod proto {
    include!("generated/proto.rs");
}

use clap::{Parser, Subcommand};
use config::AgentConfig;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

#[derive(Parser)]
#[command(name = "rust-agent")]
#[command(about = "哪吒监控 Agent (Rust 高性能版)", long_about = None)]
#[command(version)]
struct Cli {
    /// 配置文件路径
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 编辑配置文件
    Edit {
        /// 配置文件路径
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
    /// 服务操作 (install/uninstall/start/stop/restart)
    Service {
        /// 操作类型
        action: String,
        /// 配置文件路径
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
}

fn default_config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        exe.parent().unwrap_or(Path::new(".")).join("config.yml")
    } else {
        PathBuf::from("config.yml")
    }
}

fn init_logger(debug: bool) {
    let filter = if debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::time())
        .compact()
        .init();
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Edit { config }) => {
            let path = config.clone().unwrap_or_else(default_config_path);
            let agent_config = AgentConfig::read(&path).unwrap_or_default();
            println!("当前配置文件: {:?}", path);
            println!("Server: {}", agent_config.server);
            println!("UUID: {}", agent_config.uuid);
            println!("TLS: {}", agent_config.tls);
            println!("Debug: {}", agent_config.debug);
            println!("\n请手动编辑配置文件: {:?}", path);
        }
        Some(Commands::Service { action, config }) => {
            let path = config.clone().unwrap_or_else(default_config_path);
            handle_service_command(action, &path);
        }
        None => {
            let config_path = cli.config.unwrap_or_else(default_config_path);
            run_agent(config_path);
        }
    }
}

fn run_agent(config_path: PathBuf) {
    let agent_config = match AgentConfig::read(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("配置加载失败: {}", e);
            std::process::exit(1);
        }
    };

    init_logger(agent_config.debug);
    info!("rust-agent v{} starting...", monitor::VERSION);
    info!("Config: {:?}", config_path);

    let config = Arc::new(agent_config);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    runtime.block_on(async {
        client::run(config).await;
    });
}

fn handle_service_command(action: &str, config_path: &Path) {
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("rust-agent"));
    let exe_name = exe_path.file_name().unwrap_or_default().to_string_lossy();
    let config_flag = format!("-c {}", config_path.display());

    match action {
        "install" => {
            println!("Installing service: {}", exe_name);
            let service_content = format!(
                r#"[Unit]
Description=Nezha Agent (Rust)
After=network.target

[Service]
Type=simple
ExecStart={} {}
Restart=always
RestartSec=5
WorkingDirectory={}

[Install]
WantedBy=multi-user.target
"#,
                exe_path.display(),
                config_flag,
                exe_path.parent().unwrap_or(Path::new("/opt")).display()
            );

            let service_path = format!("/etc/systemd/system/{}.service", exe_name);
            match std::fs::write(&service_path, service_content) {
                Ok(_) => {
                    println!("Service file written to {}", service_path);
                    let _ = std::process::Command::new("systemctl")
                        .args(["daemon-reload"])
                        .status();
                    let _ = std::process::Command::new("systemctl")
                        .args(["enable", &exe_name])
                        .status();
                    let _ = std::process::Command::new("systemctl")
                        .args(["start", &exe_name])
                        .status();
                    println!("Service installed and started");
                }
                Err(e) => eprintln!("Failed to write service file: {}", e),
            }
        }
        "uninstall" => {
            println!("Uninstalling service: {}", exe_name);
            let _ = std::process::Command::new("systemctl")
                .args(["stop", &exe_name])
                .status();
            let _ = std::process::Command::new("systemctl")
                .args(["disable", &exe_name])
                .status();
            let service_path = format!("/etc/systemd/system/{}.service", exe_name);
            let _ = std::fs::remove_file(&service_path);
            let _ = std::process::Command::new("systemctl")
                .args(["daemon-reload"])
                .status();
            println!("Service uninstalled");
        }
        "start" => {
            let _ = std::process::Command::new("systemctl")
                .args(["start", &exe_name])
                .status();
            println!("Service started");
        }
        "stop" => {
            let _ = std::process::Command::new("systemctl")
                .args(["stop", &exe_name])
                .status();
            println!("Service stopped");
        }
        "restart" => {
            let _ = std::process::Command::new("systemctl")
                .args(["restart", &exe_name])
                .status();
            println!("Service restarted");
        }
        _ => eprintln!(
            "未知操作: {}. 支持: install/uninstall/start/stop/restart",
            action
        ),
    }
}
