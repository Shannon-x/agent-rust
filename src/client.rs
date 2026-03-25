use crate::auth::AuthInterceptor;
use crate::config::AgentConfig;
use crate::ip;
use crate::monitor;
use crate::proto;
use crate::proto::nezha_service_client::NezhaServiceClient;
use crate::tasks;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tracing::{error, info, warn};

const DELAY_WHEN_ERROR: Duration = Duration::from_secs(10);
const NETWORK_TIMEOUT: Duration = Duration::from_secs(5);

static PREV_DASHBOARD_BOOT_TIME: AtomicU64 = AtomicU64::new(0);
static GEOIP_REPORTED: AtomicBool = AtomicBool::new(false);

/// Type alias for our authenticated gRPC client
pub type AuthedClient = NezhaServiceClient<InterceptedService<Channel, AuthInterceptor>>;

/// Main agent run loop with reconnection logic
pub async fn run(config: Arc<AgentConfig>) {
    let (_reload_tx, mut reload_rx) = watch::channel(false);

    loop {
        info!("Connecting to {} ...", config.server);

        // Build channel with TLS options
        let channel = match build_channel(&config).await {
            Ok(ch) => ch,
            Err(e) => {
                error!("与面板建立连接失败: {}", e);
                tokio::time::sleep(DELAY_WHEN_ERROR).await;
                continue;
            }
        };

        // Create authenticated client
        let interceptor = AuthInterceptor::new(config.client_secret.clone(), config.uuid.clone());
        let mut client: AuthedClient = NezhaServiceClient::with_interceptor(channel, interceptor);

        info!("Connection to {} established", config.server);

        // Report system info
        let host_info = monitor::get_host(&config);
        let receipt = match tokio::time::timeout(
            NETWORK_TIMEOUT,
            client.report_system_info2(tonic::Request::new(host_info)),
        )
        .await
        {
            Ok(Ok(resp)) => resp.into_inner(),
            Ok(Err(e)) => {
                error!("上报系统信息失败: {}", e);
                tokio::time::sleep(DELAY_WHEN_ERROR).await;
                continue;
            }
            Err(_) => {
                error!("上报系统信息超时");
                tokio::time::sleep(DELAY_WHEN_ERROR).await;
                continue;
            }
        };

        // Update dashboard boot time tracking
        let prev_boot = PREV_DASHBOARD_BOOT_TIME.load(Ordering::Relaxed);
        let current_boot = receipt.data;
        let should_reset_geoip = prev_boot == 0 || current_boot != prev_boot;
        if should_reset_geoip {
            GEOIP_REPORTED.store(false, Ordering::Relaxed);
        }
        PREV_DASHBOARD_BOOT_TIME.store(current_boot, Ordering::Relaxed);

        // Create cancellation token
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let config_clone = config.clone();
        let client_for_tasks = client.clone();
        let client_for_state = client.clone();

        // Start task receiver daemon
        let cancel_rx_tasks = cancel_rx.clone();
        let task_handle = tokio::spawn({
            let config = config_clone.clone();
            let client = client_for_tasks;
            async move {
                receive_tasks_daemon(client, config, cancel_rx_tasks).await;
            }
        });

        // Start state reporter daemon
        let cancel_rx_state = cancel_rx.clone();
        let state_handle = tokio::spawn({
            let config = config_clone.clone();
            let mut client = client_for_state;
            async move {
                report_state_daemon(&mut client, config, cancel_rx_state).await;
            }
        });

        // Wait for cancellation or reload
        tokio::select! {
            _ = task_handle => {
                warn!("Task daemon exited");
            }
            _ = state_handle => {
                warn!("State daemon exited");
            }
            _ = reload_rx.changed() => {
                info!("Reloading...");
            }
        }

        // Cancel all workers
        let _ = cancel_tx.send(true);
        info!("Worker exit, reconnecting in {:?}...", DELAY_WHEN_ERROR);
        tokio::time::sleep(DELAY_WHEN_ERROR).await;
    }
}

async fn build_channel(config: &AgentConfig) -> anyhow::Result<Channel> {
    let server_addr = if config.server.contains("://") {
        config.server.clone()
    } else if config.tls {
        format!("https://{}", config.server)
    } else {
        format!("http://{}", config.server)
    };

    let mut endpoint = Endpoint::from_shared(server_addr)?
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .http2_keep_alive_interval(Duration::from_secs(30))
        .keep_alive_timeout(Duration::from_secs(10))
        .keep_alive_while_idle(true);

    if config.tls {
        let mut tls_config = ClientTlsConfig::new();
        if config.insecure_tls {
            tls_config = tls_config.with_native_roots();
        }
        endpoint = endpoint.tls_config(tls_config)?;
    }

    let channel = endpoint.connect().await?;
    Ok(channel)
}

/// Receive and dispatch tasks from the dashboard
async fn receive_tasks_daemon(
    mut client: AuthedClient,
    config: Arc<AgentConfig>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    // Create bidirectional task stream
    let (task_result_tx, task_result_rx) = tokio::sync::mpsc::channel::<proto::TaskResult>(32);

    let outbound = tokio_stream::wrappers::ReceiverStream::new(task_result_rx);

    let response = match client.request_task(outbound).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("请求任务失败: {}", e);
            return;
        }
    };

    let mut inbound = response.into_inner();

    loop {
        tokio::select! {
            _ = cancel_rx.changed() => {
                info!("Task receiver cancelled");
                return;
            }
            task = inbound.message() => {
                match task {
                    Ok(Some(t)) => {
                        let config = config.clone();
                        let tx = task_result_tx.clone();
                        let task_client = client.clone();
                        tokio::spawn(async move {
                            if let Some(result) = tasks::do_task(&t, &config, task_client).await {
                                if let Err(e) = tx.send(result).await {
                                    error!("send task result failed: {}", e);
                                }
                            }
                        });
                    }
                    Ok(None) => {
                        info!("Task stream ended");
                        return;
                    }
                    Err(e) => {
                        error!("receiveTasks exit: {}", e);
                        return;
                    }
                }
            }
        }
    }
}

/// Periodically report system state to the dashboard
async fn report_state_daemon(
    client: &mut AuthedClient,
    config: Arc<AgentConfig>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    let (state_tx, state_rx) = tokio::sync::mpsc::channel::<proto::State>(8);
    let outbound = tokio_stream::wrappers::ReceiverStream::new(state_rx);

    let response = match client.report_system_state(outbound).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("上报状态信息失败: {}", e);
            return;
        }
    };

    let mut inbound = response.into_inner();

    let mut last_report_host = Instant::now() - Duration::from_secs(700);
    let mut last_report_ip = Instant::now() - Duration::from_secs(3600);

    loop {
        tokio::select! {
            _ = cancel_rx.changed() => {
                info!("State reporter cancelled");
                return;
            }
            _ = tokio::time::sleep(Duration::from_secs(config.report_delay as u64)) => {
                // Track network speed
                monitor::track_network_speed(&config.nic_allowlist);

                // Send state
                let state = monitor::get_state(&config);
                if let Err(e) = state_tx.send(state).await {
                    error!("send state failed: {}", e);
                    return;
                }

                // Receive receipt
                match tokio::time::timeout(Duration::from_secs(10), inbound.message()).await {
                    Ok(Ok(Some(_))) => {} // Receipt received
                    Ok(Ok(None)) => {
                        info!("State stream ended");
                        return;
                    }
                    Ok(Err(e)) => {
                        error!("reportState recv error: {}", e);
                        return;
                    }
                    Err(_) => {
                        error!("reportState recv timeout");
                        return;
                    }
                }

                // Periodically re-report host info (every 10 minutes)
                if last_report_host.elapsed() > Duration::from_secs(600) {
                    let host_info = monitor::get_host(&config);
                    let mut report_client = client.clone();
                    match report_client.report_system_info2(tonic::Request::new(host_info)).await {
                        Ok(receipt) => {
                            let boot_time = receipt.into_inner().data;
                            let prev = PREV_DASHBOARD_BOOT_TIME.load(Ordering::Relaxed);
                            if prev > 0 && boot_time != prev {
                                GEOIP_REPORTED.store(false, Ordering::Relaxed);
                            }
                            PREV_DASHBOARD_BOOT_TIME.store(boot_time, Ordering::Relaxed);
                            last_report_host = Instant::now();
                        }
                        Err(e) => {
                            warn!("ReportSystemInfo2 error: {}", e);
                        }
                    }
                }

                // Report GeoIP
                let ip_period = Duration::from_secs(config.ip_report_period as u64);
                let geoip_reported = GEOIP_REPORTED.load(Ordering::Relaxed);
                if last_report_ip.elapsed() > ip_period || !geoip_reported {
                    if let Some(pbg) = ip::fetch_ip(
                        config.use_ipv6_country_code,
                        &config.custom_ip_api,
                    ).await {
                        if ip::GEO_QUERY_IP_CHANGED.load(Ordering::Relaxed) || !geoip_reported {
                            let mut geo_client = client.clone();
                            match geo_client.report_geo_ip(tonic::Request::new(pbg)).await {
                                Ok(resp) => {
                                    let geoip = resp.into_inner();
                                    PREV_DASHBOARD_BOOT_TIME.store(geoip.dashboard_boot_time, Ordering::Relaxed);
                                    ip::set_cached_country_code(&geoip.country_code);
                                    ip::GEO_QUERY_IP_CHANGED.store(false, Ordering::Relaxed);
                                    GEOIP_REPORTED.store(true, Ordering::Relaxed);
                                    last_report_ip = Instant::now();
                                }
                                Err(e) => {
                                    warn!("ReportGeoIP error: {}", e);
                                }
                            }
                        } else {
                            last_report_ip = Instant::now();
                        }
                    }
                }
            }
        }
    }
}
