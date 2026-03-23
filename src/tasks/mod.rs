pub mod command;
pub mod config_task;
pub mod fm;
pub mod http_get;
pub mod icmp_ping;
pub mod nat;
pub mod tcp_ping;
pub mod terminal;

use crate::config::AgentConfig;
use crate::proto;
use tracing::warn;

/// Task type constants matching Go's iota
pub const TASK_TYPE_HTTP_GET: u64 = 1;
pub const TASK_TYPE_ICMP_PING: u64 = 2;
pub const TASK_TYPE_TCP_PING: u64 = 3;
pub const TASK_TYPE_COMMAND: u64 = 4;
pub const _TASK_TYPE_TERMINAL: u64 = 5;
pub const TASK_TYPE_UPGRADE: u64 = 6;
pub const TASK_TYPE_KEEPALIVE: u64 = 7;
pub const TASK_TYPE_TERMINAL_GRPC: u64 = 8;
pub const TASK_TYPE_NAT: u64 = 9;
pub const _TASK_TYPE_REPORT_HOST_INFO_DEPRECATED: u64 = 10;
pub const TASK_TYPE_FM: u64 = 11;
pub const TASK_TYPE_REPORT_CONFIG: u64 = 12;
pub const TASK_TYPE_APPLY_CONFIG: u64 = 13;

/// Dispatch and execute a task, returning the result (if any)
#[allow(clippy::field_reassign_with_default)]
pub async fn do_task(task: &proto::Task, config: &AgentConfig) -> Option<proto::TaskResult> {
    let mut result = proto::TaskResult {
        id: task.id,
        r#type: task.r#type,
        ..Default::default()
    };

    match task.r#type {
        TASK_TYPE_HTTP_GET => {
            http_get::handle(task, &mut result, config).await;
        }
        TASK_TYPE_ICMP_PING => {
            icmp_ping::handle(task, &mut result, config).await;
        }
        TASK_TYPE_TCP_PING => {
            tcp_ping::handle(task, &mut result, config).await;
        }
        TASK_TYPE_COMMAND => {
            command::handle(task, &mut result, config).await;
        }
        TASK_TYPE_UPGRADE => {
            tracing::info!("Upgrade task received, skipping in Rust agent");
            result.data = "Rust agent does not support self-update".to_string();
        }
        TASK_TYPE_TERMINAL_GRPC => {
            terminal::handle(task, config).await;
            return None;
        }
        TASK_TYPE_NAT => {
            nat::handle(task, config).await;
            return None;
        }
        TASK_TYPE_FM => {
            fm::handle(task, config).await;
            return None;
        }
        TASK_TYPE_REPORT_CONFIG => {
            config_task::handle_report(config, &mut result);
        }
        TASK_TYPE_APPLY_CONFIG => {
            config_task::handle_apply(task, config).await;
            return None;
        }
        TASK_TYPE_KEEPALIVE => {}
        _ => {
            warn!("不支持的任务类型: {}", task.r#type);
            return None;
        }
    }

    Some(result)
}
