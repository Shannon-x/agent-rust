use crate::config::AgentConfig;
use crate::proto;
use std::time::Instant;
use tokio::process::Command;
use tracing::info;

pub async fn handle(task: &proto::Task, result: &mut proto::TaskResult, config: &AgentConfig) {
    if config.disable_command_execute {
        result.data = "此 Agent 已禁止命令执行".to_string();
        return;
    }

    info!("Executing command task");
    let started_at = Instant::now();

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(7200), // 2 hours
        Command::new("sh")
            .arg("-c")
            .arg(&task.data)
            .env_clear()
            .envs(std::env::vars())
            .output(),
    )
    .await;

    match output {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                result.data = stdout;
                result.successful = true;
            } else {
                result.data = format!("{}\n{}", stdout, stderr);
            }
        }
        Ok(Err(e)) => {
            result.data = e.to_string();
        }
        Err(_) => {
            result.data = "任务执行超时\n".to_string();
        }
    }

    result.delay = started_at.elapsed().as_secs_f32();
}
