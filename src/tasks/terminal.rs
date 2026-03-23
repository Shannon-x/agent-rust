use crate::config::AgentConfig;
use crate::proto;
use serde::Deserialize;
use tracing::warn;

#[derive(Deserialize)]
struct TerminalTask {
    #[serde(rename = "StreamID")]
    stream_id: String,
}

pub async fn handle(task: &proto::Task, config: &AgentConfig) {
    if config.disable_command_execute {
        warn!("此 Agent 已禁止命令执行");
        return;
    }

    let terminal: TerminalTask = match serde_json::from_str(&task.data) {
        Ok(t) => t,
        Err(e) => {
            warn!("Terminal 任务解析错误: {}", e);
            return;
        }
    };

    tracing::info!("Terminal task received: {}", terminal.stream_id);
    // TODO: Full PTY + bidirectional IOStream implementation
    // Requires creating a separate IOStream gRPC call with the same auth
    warn!("Terminal streaming not yet fully implemented in Rust agent");
}
