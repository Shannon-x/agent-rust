use crate::config::AgentConfig;
use crate::proto;
use std::time::Instant;
use tracing::info;

const USER_AGENT: &str = "nezha-agent/1.0";

pub async fn handle(task: &proto::Task, result: &mut proto::TaskResult, config: &AgentConfig) {
    if config.disable_send_query {
        result.data = "This server has disabled query sending".to_string();
        return;
    }

    let url = &task.data;
    info!("HTTP-GET Task: {}", url);

    let start = Instant::now();

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(USER_AGENT)
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            result.data = e.to_string();
            return;
        }
    };

    match client.get(url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            match resp.bytes().await {
                Ok(_) => {
                    result.delay = start.elapsed().as_micros() as f32 / 1000.0;
                    if (200..400).contains(&status) {
                        result.successful = true;
                    } else {
                        result.data = format!("\n应用错误: {}", status);
                    }
                }
                Err(e) => {
                    result.data = e.to_string();
                }
            }
        }
        Err(e) => {
            result.data = e.to_string();
        }
    }
}
