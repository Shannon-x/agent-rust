use crate::config::AgentConfig;
use crate::proto;
use crate::util;
use std::net::IpAddr;
use std::time::Duration;
use surge_ping::{Client, Config, PingIdentifier, PingSequence};
use tracing::info;

pub async fn handle(task: &proto::Task, result: &mut proto::TaskResult, config: &AgentConfig) {
    if config.disable_send_query {
        result.data = "This server has disabled query sending".to_string();
        return;
    }

    let target = &task.data;
    let ip_str = match util::lookup_ip(target).await {
        Ok(ip) => ip,
        Err(e) => {
            result.data = e.to_string();
            return;
        }
    };

    info!("ICMP-Ping Task: Pinging {}({})", target, ip_str);

    let ip: IpAddr = match ip_str.parse() {
        Ok(ip) => ip,
        Err(e) => {
            result.data = e.to_string();
            return;
        }
    };

    let config_builder = Config::default();
    let client = match Client::new(&config_builder) {
        Ok(c) => c,
        Err(e) => {
            result.data = format!("Failed to create ICMP client: {}", e);
            return;
        }
    };

    let mut pinger = client.pinger(ip, PingIdentifier(rand::random())).await;
    pinger.timeout(Duration::from_secs(4));

    let mut total_rtt = Duration::ZERO;
    let mut success_count = 0u32;

    for seq in 0..5u16 {
        match pinger.ping(PingSequence(seq), &[]).await {
            Ok((_, rtt)) => {
                total_rtt += rtt;
                success_count += 1;
            }
            Err(_) => continue,
        }
    }

    if success_count == 0 {
        result.data = "pockets recv 0".to_string();
        return;
    }

    let avg_rtt = total_rtt / success_count;
    result.delay = avg_rtt.as_micros() as f32 / 1000.0;
    result.successful = true;
}
