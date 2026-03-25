use crate::client::AuthedClient;
use crate::config::AgentConfig;
use crate::proto;
use futures_util::StreamExt;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde::Deserialize;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info, warn};

#[derive(Deserialize)]
struct TerminalTask {
    #[serde(rename = "StreamID")]
    stream_id: String,
}

pub async fn handle(
    task: &proto::Task,
    config: &AgentConfig,
    mut client: AuthedClient,
) {
    if config.disable_command_execute {
        warn!("此 Agent 已禁止命令执行 (Web终端被拒绝)");
        return;
    }

    let terminal: TerminalTask = match serde_json::from_str(&task.data) {
        Ok(t) => t,
        Err(e) => {
            warn!("Terminal 任务解析错误: {}", e);
            return;
        }
    };

    info!("Starting Web Terminal session: {}", terminal.stream_id);

    // Setup PTY system
    let pty_system = NativePtySystem::default();
    let size = PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty_pair = match pty_system.openpty(size) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to open PTY: {}", e);
            return;
        }
    };

    let cmd_line = if cfg!(target_os = "windows") {
        "cmd.exe"
    } else {
        "/bin/bash"
    };

    let mut cmd = CommandBuilder::new(cmd_line);
    cmd.env("TERM", "xterm-256color");

    let _child = match pty_pair.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to spawn shell in PTY: {}", e);
            return;
        }
    };

    // Setup gRPC IOStream
    let (tx, rx) = mpsc::channel::<proto::IoStreamData>(32);
    let outbound = ReceiverStream::new(rx);

    let response = match client.io_stream(outbound).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to establish IOStream for terminal: {}", e);
            return;
        }
    };

    let mut inbound = response.into_inner();

    // The first packet must be the Magic Bytes + StreamID
    let mut magic_header = vec![0xff, 0x05, 0xff, 0x05];
    magic_header.extend_from_slice(terminal.stream_id.as_bytes());
    if tx.send(proto::IoStreamData { data: magic_header }).await.is_err() {
        error!("Failed to send terminal magic header");
        return;
    }

    // Split PTY IO into async readers/writers using tokio blocking adapters
    // portable_pty provides std::io traits, we need tokio::io
    let mut pty_reader = tokio::task::spawn_blocking({
        let pty_reader = pty_pair.master.try_clone_reader().unwrap();
        move || {
            let mut std_reader = pty_reader;
            let (async_tx, async_rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match std::io::Read::read(&mut std_reader, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if async_tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
            async_rx
        }
    })
    .await
    .unwrap();

    let mut pty_writer = pty_pair.master.take_writer().unwrap();

    let stream_id = terminal.stream_id.clone();
    let tx_clone = tx.clone();
    
    // Spawn a task to read from PTY and send to gRPC
    let pty_to_grpc = tokio::spawn(async move {
        // Since we got a std::sync::mpsc::Receiver from the blocking task, we poll it in a loop
        loop {
            // we use try_recv to not block the async executor, with a small sleep
            match pty_reader.try_recv() {
                Ok(data) => {
                    if tx_clone.send(proto::IoStreamData { data }).await.is_err() {
                        break;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    break;
                }
            }
        }
    });

    // Spawn a task to send keepalive pings every 30s
    let tx_keepalive = tx.clone();
    let keepalive_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if tx_keepalive.send(proto::IoStreamData { data: vec![] }).await.is_err() {
                break;
            }
        }
    });

    // Read from gRPC and write to PTY
    loop {
        match inbound.next().await {
            Some(Ok(msg)) => {
                if msg.data.is_empty() {
                    // Ignored (heartbeat from panel)
                    continue;
                }
                // Write to PTY
                let data = msg.data;
                if let Err(e) = tokio::task::spawn_blocking(move || {
                    let mut writer = pty_writer;
                    std::io::Write::write_all(&mut writer, &data)
                })
                .await
                .unwrap()
                {
                    error!("Failed to write to PTY: {}", e);
                    break;
                }
            }
            Some(Err(e)) => {
                error!("Terminal IOStream error: {}", e);
                break;
            }
            None => {
                info!("Terminal IOStream closed by server");
                break;
            }
        }
    }

    // Cleanup
    pty_to_grpc.abort();
    keepalive_task.abort();
    info!("Terminal session {} ended.", stream_id);
}
