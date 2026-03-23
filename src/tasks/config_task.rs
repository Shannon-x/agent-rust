use crate::config::AgentConfig;
use crate::proto;
use tracing::{info, warn};

/// Handle report config task - serialize current config and send it back
pub fn handle_report(config: &AgentConfig, result: &mut proto::TaskResult) {
    if config.disable_command_execute {
        result.data = "此 Agent 已禁止命令执行".to_string();
        return;
    }

    info!("Executing Report Config Task");

    match serde_json::to_string(config) {
        Ok(json) => {
            result.data = json;
            result.successful = true;
        }
        Err(e) => {
            result.data = e.to_string();
        }
    }
}

/// Handle apply config task - parse and apply new configuration
pub async fn handle_apply(task: &proto::Task, config: &AgentConfig) {
    if config.disable_command_execute {
        return;
    }

    info!("Executing Apply Config Task");

    let mut new_config: AgentConfig = match serde_json::from_str(&task.data) {
        Ok(c) => c,
        Err(e) => {
            warn!("Parsing Config failed: {}", e);
            return;
        }
    };

    // Preserve file path from current config
    new_config.file_path = config.file_path.clone();

    if let Err(e) = new_config.validate(true) {
        warn!("Validate Config failed: {}", e);
        return;
    }

    info!("Will reload workers in 10 seconds");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    info!("Applying new configuration...");
    if let Err(e) = new_config.save() {
        warn!("Save config failed: {}", e);
    }
    // The main loop will reload on next iteration
}
