use crate::config::AgentConfig;
use crate::proto;
use crate::util::USER_AGENT;
use std::time::Instant;
use tracing::info;

pub async fn handle(task: &proto::Task, result: &mut proto::TaskResult, config: &AgentConfig) {
    if config.disable_send_query {
        result.data = "This server has disabled query sending".to_string();
        return;
    }

    let url = &task.data;
    info!("HTTP-GET Task: {}", url);

    let start = Instant::now();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(USER_AGENT)
        .danger_accept_invalid_certs(true)
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            result.data = e.to_string();
            return;
        }
    };

    match client.get(url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();

            // Extract TLS certificate info if available
            // Note: reqwest doesn't directly expose TLS cert info like Go does
            // We'll report what we can

            // Read body to completion
            match resp.bytes().await {
                Ok(_) => {
                    result.delay = start.elapsed().as_micros() as f32 / 1000.0;
                    if status >= 200 && status < 400 {
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
