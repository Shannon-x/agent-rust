use crate::config::AgentConfig;
use crate::proto;
use serde::Deserialize;
use tracing::warn;

#[derive(Deserialize)]
struct TaskFm {
    #[serde(rename = "StreamID")]
    stream_id: String,
}

pub async fn handle(task: &proto::Task, config: &AgentConfig) {
    if config.disable_command_execute {
        warn!("此 Agent 已禁止命令执行");
        return;
    }

    let fm_task: TaskFm = match serde_json::from_str(&task.data) {
        Ok(f) => f,
        Err(e) => {
            warn!("FM 任务解析错误: {}", e);
            return;
        }
    };

    tracing::info!("FM task received: {}", fm_task.stream_id);
    // TODO: Full FM protocol over IOStream
    warn!("FM streaming not yet fully implemented in Rust agent");
}
