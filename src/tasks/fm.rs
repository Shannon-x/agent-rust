use crate::client::AuthedClient;
use crate::config::AgentConfig;
use crate::fm_proto;
use crate::proto;
use futures_util::StreamExt;
use serde::Deserialize;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info, warn};

#[derive(Deserialize, Debug)]
struct TaskFm {
    #[serde(rename = "StreamID")]
    stream_id: String,
}

#[derive(Deserialize, Debug)]
struct FmAction {
    action: String,
    path: Option<String>,
    old_path: Option<String>,
    new_path: Option<String>,
    name: Option<String>,
    uid: Option<String>,
    gid: Option<String>,
    size: Option<u64>,
}

pub async fn handle(task: &proto::Task, config: &AgentConfig, mut client: AuthedClient) {
    if config.disable_command_execute {
        warn!("此 Agent 已禁止命令执行 (文件管理被拒绝)");
        return;
    }

    let fm_task: TaskFm = match serde_json::from_str(&task.data) {
        Ok(f) => f,
        Err(e) => {
            warn!("FM 任务解析错误: {}", e);
            return;
        }
    };

    info!("Starting File Manager session: {}", fm_task.stream_id);

    // Setup gRPC IOStream
    let (tx, rx) = mpsc::channel::<proto::IoStreamData>(32);
    let outbound = ReceiverStream::new(rx);

    let response = match client.io_stream(outbound).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to establish IOStream for FM: {}", e);
            return;
        }
    };

    let mut inbound = response.into_inner();

    // The first packet must be the Magic Bytes + StreamID
    let mut magic_header = vec![0xff, 0x05, 0xff, 0x05];
    magic_header.extend_from_slice(fm_task.stream_id.as_bytes());
    if tx
        .send(proto::IoStreamData { data: magic_header })
        .await
        .is_err()
    {
        error!("Failed to send FM magic header");
        return;
    }

    // Spawn keepalive task (every 30s send empty packet)
    let tx_keepalive = tx.clone();
    let keepalive_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if tx_keepalive
                .send(proto::IoStreamData { data: vec![] })
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let root_path = PathBuf::from("/");
    let tx_main = tx.clone();

    tokio::spawn(async move {
        loop {
            match inbound.next().await {
                Some(Ok(msg)) => {
                    if msg.data.is_empty() {
                        // Heartbeat
                        continue;
                    }

                    // The incoming data is a JSON string of FmAction
                    let json_str = String::from_utf8_lossy(&msg.data);

                    let action: FmAction = match serde_json::from_str(&json_str) {
                        Ok(a) => a,
                        Err(e) => {
                            let _ = tx_main
                                .send(proto::IoStreamData {
                                    data: fm_proto::create_error(&format!("Invalid JSON: {}", e)),
                                })
                                .await;
                            continue;
                        }
                    };

                    match action.action.as_str() {
                        "list" => {
                            if let Some(path_str) = action.path {
                                let target = resolve_safe_path(&root_path, &path_str);
                                handle_list(&target, path_str, &tx_main).await;
                            }
                        }
                        "read" => {
                            if let Some(path_str) = action.path {
                                let target = resolve_safe_path(&root_path, &path_str);
                                handle_read(&target, &tx_main).await;
                            }
                        }
                        "update" | "create" => {
                            if let (Some(path_str), Some(size)) = (action.path, action.size) {
                                let target = resolve_safe_path(&root_path, &path_str);
                                handle_write(&target, size, &mut inbound, &tx_main).await;
                            }
                        }
                        "delete" => {
                            if let Some(path_str) = action.path {
                                let target = resolve_safe_path(&root_path, &path_str);
                                handle_delete(&target, &tx_main).await;
                            }
                        }
                        "mkdir" => {
                            if let (Some(path_str), Some(name)) = (action.path, action.name) {
                                let mut target = resolve_safe_path(&root_path, &path_str);
                                target.push(name);
                                handle_mkdir(&target, &tx_main).await;
                            }
                        }
                        "rename" => {
                            if let (Some(old), Some(new)) = (action.old_path, action.new_path) {
                                let old_target = resolve_safe_path(&root_path, &old);
                                let new_target = resolve_safe_path(&root_path, &new);
                                handle_rename(&old_target, &new_target, &tx_main).await;
                            }
                        }
                        "chown" => {
                            let _ = tx_main
                                .send(proto::IoStreamData {
                                    data: fm_proto::create_error("chown not fully implemented yet"),
                                })
                                .await;
                        }
                        _ => {
                            let _ = tx_main
                                .send(proto::IoStreamData {
                                    data: fm_proto::create_error("Unknown action"),
                                })
                                .await;
                        }
                    }
                }
                Some(Err(e)) => {
                    error!("FM IOStream error: {}", e);
                    break;
                }
                None => {
                    info!("FM IOStream closed by server");
                    break;
                }
            }
        }
        keepalive_task.abort();
    });
}

fn resolve_safe_path(root: &Path, rel: &str) -> PathBuf {
    // Basic protection (production agent typically binds to root `/` anyway)
    let p = root.join(rel.trim_start_matches('/'));
    // we use clean to prevent simple `../` tricks, but for an agent with full root access, canonicalization is better.
    // However, canonicalize fails if the file doesn't exist yet (e.g., in create/mkdir).
    // So we just return the joined path for now.
    p
}

async fn handle_list(target: &Path, original_path: String, tx: &mpsc::Sender<proto::IoStreamData>) {
    let mut dir = match fs::read_dir(target).await {
        Ok(d) => d,
        Err(e) => {
            let _ = tx
                .send(proto::IoStreamData {
                    data: fm_proto::create_error(&e.to_string()),
                })
                .await;
            return;
        }
    };

    let mut buf = fm_proto::create_dir_header(&original_path);
    while let Ok(Some(entry)) = dir.next_entry().await {
        if let Ok(metadata) = entry.metadata().await {
            let name = entry.file_name().to_string_lossy().to_string();
            fm_proto::append_filename(&mut buf, &name, metadata.is_dir());
        }
    }

    let _ = tx.send(proto::IoStreamData { data: buf }).await;
    let _ = tx
        .send(proto::IoStreamData {
            data: fm_proto::COMPLETE_IDENTIFIER.to_vec(),
        })
        .await;
}

async fn handle_read(target: &Path, tx: &mpsc::Sender<proto::IoStreamData>) {
    let mut file = match fs::File::open(target).await {
        Ok(f) => f,
        Err(e) => {
            let _ = tx
                .send(proto::IoStreamData {
                    data: fm_proto::create_error(&e.to_string()),
                })
                .await;
            return;
        }
    };

    let size = match file.metadata().await {
        Ok(m) => m.len(),
        Err(_) => 0,
    };

    let header = fm_proto::create_file_header(size);
    if tx.send(proto::IoStreamData { data: header }).await.is_err() {
        return;
    }

    let mut buf = vec![0u8; 64 * 1024]; // 64KB chunks
    loop {
        match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let mut chunk = fm_proto::create_file_header(n as u64);
                chunk.extend_from_slice(&buf[..n]);
                if tx.send(proto::IoStreamData { data: chunk }).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                let _ = tx
                    .send(proto::IoStreamData {
                        data: fm_proto::create_error(&e.to_string()),
                    })
                    .await;
                break;
            }
        }
    }
    // send end of stream packet
    let _ = tx
        .send(proto::IoStreamData {
            data: fm_proto::COMPLETE_IDENTIFIER.to_vec(),
        })
        .await;
}

async fn handle_write(
    target: &Path,
    total_size: u64,
    inbound: &mut tonic::codec::Streaming<proto::IoStreamData>,
    tx: &mpsc::Sender<proto::IoStreamData>,
) {
    let mut file = match fs::File::create(target).await {
        Ok(f) => f,
        Err(e) => {
            let _ = tx
                .send(proto::IoStreamData {
                    data: fm_proto::create_error(&e.to_string()),
                })
                .await;
            return;
        }
    };

    let mut received = 0u64;
    while received < total_size {
        match inbound.next().await {
            Some(Ok(msg)) => {
                // Ignore headers if panel sends them wrapped, but panel actually sends raw bytes for upload stream
                if !msg.data.is_empty() {
                    if let Err(e) = file.write_all(&msg.data).await {
                        let _ = tx
                            .send(proto::IoStreamData {
                                data: fm_proto::create_error(&e.to_string()),
                            })
                            .await;
                        return;
                    }
                    received += msg.data.len() as u64;
                }
            }
            Some(Err(e)) => {
                let _ = tx
                    .send(proto::IoStreamData {
                        data: fm_proto::create_error(&e.to_string()),
                    })
                    .await;
                return;
            }
            None => break,
        }
    }
    let _ = file.flush().await;
}

async fn handle_delete(target: &Path, tx: &mpsc::Sender<proto::IoStreamData>) {
    let result = if target.is_dir() {
        fs::remove_dir_all(target).await
    } else {
        fs::remove_file(target).await
    };

    if let Err(e) = result {
        let _ = tx
            .send(proto::IoStreamData {
                data: fm_proto::create_error(&e.to_string()),
            })
            .await;
    }
}

async fn handle_mkdir(target: &Path, tx: &mpsc::Sender<proto::IoStreamData>) {
    if let Err(e) = fs::create_dir_all(target).await {
        let _ = tx
            .send(proto::IoStreamData {
                data: fm_proto::create_error(&e.to_string()),
            })
            .await;
    }
}

async fn handle_rename(old: &Path, new: &Path, tx: &mpsc::Sender<proto::IoStreamData>) {
    if let Err(e) = fs::rename(old, new).await {
        let _ = tx
            .send(proto::IoStreamData {
                data: fm_proto::create_error(&e.to_string()),
            })
            .await;
    }
}
