use crate::config::AgentConfig;
use crate::proto;
use serde::Deserialize;
use tracing::warn;

#[derive(Deserialize)]
struct TaskNat {
    #[serde(rename = "StreamID")]
    stream_id: String,
    #[serde(rename = "Host")]
    host: String,
}

pub async fn handle(task: &proto::Task, config: &AgentConfig) {
    if config.disable_nat {
        warn!("This server has disabled NAT traversal");
        return;
    }

    let nat: TaskNat = match serde_json::from_str(&task.data) {
        Ok(n) => n,
        Err(e) => {
            warn!("NAT 任务解析错误: {}", e);
            return;
        }
    };

    tracing::info!("NAT task received: {} -> {}", nat.stream_id, nat.host);
    // TODO: Full TCP ↔ IOStream bidirectional forwarding
    warn!("NAT streaming not yet fully implemented in Rust agent");
}
