use crate::config::AgentConfig;
use crate::proto;
use crate::util;
use std::time::Instant;
use tokio::net::TcpStream;
use tracing::info;

pub async fn handle(task: &proto::Task, result: &mut proto::TaskResult, config: &AgentConfig) {
    if config.disable_send_query {
        result.data = "This server has disabled query sending".to_string();
        return;
    }

    // Parse host:port
    let data = &task.data;
    let (host, port) = match data.rsplit_once(':') {
        Some((h, p)) => (h, p),
        None => {
            result.data = format!("invalid address format: {}", data);
            return;
        }
    };

    // Resolve hostname
    let ip_addr = match util::lookup_ip(host).await {
        Ok(ip) => ip,
        Err(e) => {
            result.data = e.to_string();
            return;
        }
    };

    let addr = format!("{}:{}", ip_addr, port);
    info!("TCP-Ping Task: Pinging {}", addr);

    let start = Instant::now();
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(_conn)) => {
            result.delay = start.elapsed().as_micros() as f32 / 1000.0;
            result.successful = true;
        }
        Ok(Err(e)) => {
            result.data = e.to_string();
        }
        Err(_) => {
            result.data = "connection timeout".to_string();
        }
    }
}
