pub mod conn;
pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod load;
pub mod memory;
pub mod network;
pub mod temperature;

use crate::config::AgentConfig;
use crate::proto;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;
use tracing::warn;

static NET_IN_SPEED: AtomicU64 = AtomicU64::new(0);
static NET_OUT_SPEED: AtomicU64 = AtomicU64::new(0);
static NET_IN_TRANSFER: AtomicU64 = AtomicU64::new(0);
static NET_OUT_TRANSFER: AtomicU64 = AtomicU64::new(0);
static LAST_UPDATE_NET_STATS: AtomicU64 = AtomicU64::new(0);
static CACHED_BOOT_TIME: AtomicU64 = AtomicU64::new(0);

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Collect host hardware information (static info, called infrequently)
#[allow(clippy::field_reassign_with_default)]
pub fn get_host(config: &AgentConfig) -> proto::Host {
    let mut host = proto::Host::default();
    host.version = VERSION.to_string();

    // Platform info — read directly from /etc/os-release for zero overhead
    host.platform = read_os_field("NAME");
    host.platform_version = read_os_field("VERSION_ID");
    host.arch = std::env::consts::ARCH.to_string();

    // Boot time from /proc/stat
    host.boot_time = read_boot_time();
    CACHED_BOOT_TIME.store(host.boot_time, Ordering::Relaxed);

    // CPU info
    match cpu::get_host_info() {
        Ok(cpus) => host.cpu = cpus,
        Err(e) => warn!("CPU info error: {}", e),
    }

    // Memory — read /proc/meminfo directly (avoid sysinfo)
    match memory::get_memory_info() {
        Ok(mem) => {
            host.mem_total = mem.total;
            host.swap_total = mem.swap_total;
        }
        Err(e) => warn!("Memory info error: {}", e),
    }

    // Disk
    host.disk_total = disk::get_total(&config.hard_drive_partition_allowlist);

    // GPU
    if config.gpu {
        match gpu::get_host_info() {
            Ok(gpus) => host.gpu = gpus,
            Err(e) => warn!("GPU info error: {}", e),
        }
    }

    // Virtualization detection
    host.virtualization = detect_virtualization();

    host
}

/// Collect host state (dynamic metrics — called every report_delay seconds)
/// PERF: Zero sysinfo dependency, all data from /proc directly
#[allow(clippy::field_reassign_with_default)]
pub fn get_state(config: &AgentConfig) -> proto::State {
    let mut state = proto::State::default();

    // CPU usage — non-blocking, uses cached previous reading
    state.cpu = cpu::get_usage_cached();

    // Memory — direct /proc/meminfo read (< 10μs)
    match memory::get_memory_info() {
        Ok(mem) => {
            state.mem_used = mem.used;
            state.swap_used = mem.swap_used;
        }
        Err(e) => warn!("Memory error: {}", e),
    }

    // Disk
    state.disk_used = disk::get_used(&config.hard_drive_partition_allowlist);

    // Load — direct /proc/loadavg read (< 5μs)
    match load::get_load() {
        Ok((l1, l5, l15)) => {
            state.load1 = l1;
            state.load5 = l5;
            state.load15 = l15;
        }
        Err(e) => warn!("Load error: {}", e),
    }

    // Process count — read /proc/loadavg 4th field (< 5μs, no scanning)
    if !config.skip_procs_count {
        state.process_count = get_process_count_fast();
    }

    // Temperature
    if config.temperature {
        match temperature::get_temperatures() {
            Ok(temps) => {
                state.temperatures = temps
                    .into_iter()
                    .map(|(name, temp)| proto::StateSensorTemperature {
                        name,
                        temperature: temp,
                    })
                    .collect();
            }
            Err(e) => warn!("Temperature error: {}", e),
        }
    }

    // GPU usage
    if config.gpu {
        match gpu::get_usage() {
            Ok(usages) => state.gpu = usages,
            Err(e) => warn!("GPU usage error: {}", e),
        }
    }

    // Network stats (pre-computed by track_network_speed)
    state.net_in_transfer = NET_IN_TRANSFER.load(Ordering::Relaxed);
    state.net_out_transfer = NET_OUT_TRANSFER.load(Ordering::Relaxed);
    state.net_in_speed = NET_IN_SPEED.load(Ordering::Relaxed);
    state.net_out_speed = NET_OUT_SPEED.load(Ordering::Relaxed);

    // Uptime
    let boot_time = CACHED_BOOT_TIME.load(Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    state.uptime = now.saturating_sub(boot_time);

    // Connection count — direct /proc/net/ line counting
    if !config.skip_connection_count {
        match conn::get_connections() {
            Ok((tcp, udp)) => {
                state.tcp_conn_count = tcp;
                state.udp_conn_count = udp;
            }
            Err(e) => warn!("Connection count error: {}", e),
        }
    }

    state
}

/// Track network speed — called before each state report
pub fn track_network_speed(nic_allowlist: &std::collections::HashMap<String, bool>) {
    let (in_transfer, out_transfer) = match network::get_traffic(nic_allowlist) {
        Ok(v) => v,
        Err(_) => return,
    };

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let last = LAST_UPDATE_NET_STATS.load(Ordering::Relaxed);
    let diff = now.saturating_sub(last);
    if diff > 0 && last > 0 {
        let prev_in = NET_IN_TRANSFER.load(Ordering::Relaxed);
        let prev_out = NET_OUT_TRANSFER.load(Ordering::Relaxed);
        if prev_in > 0 {
            NET_IN_SPEED.store(in_transfer.saturating_sub(prev_in) / diff, Ordering::Relaxed);
            NET_OUT_SPEED.store(out_transfer.saturating_sub(prev_out) / diff, Ordering::Relaxed);
        }
    }

    NET_IN_TRANSFER.store(in_transfer, Ordering::Relaxed);
    NET_OUT_TRANSFER.store(out_transfer, Ordering::Relaxed);
    LAST_UPDATE_NET_STATS.store(now, Ordering::Relaxed);
}

/// Fast process count from /proc/loadavg (4th field = running/total)
/// Avoids scanning /proc entirely — O(1) cost
fn get_process_count_fast() -> u64 {
    if let Ok(content) = std::fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() >= 4 {
            if let Some(total) = parts[3].split('/').nth(1) {
                return total.parse().unwrap_or(0);
            }
        }
    }
    0
}

fn detect_virtualization() -> String {
    if let Ok(output) = std::process::Command::new("systemd-detect-virt").output() {
        if output.status.success() {
            let virt = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if virt != "none" {
                return virt;
            }
        }
    }
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        if cpuinfo.contains("hypervisor") {
            return "virtual".to_string();
        }
    }
    String::new()
}

fn read_boot_time() -> u64 {
    if let Ok(content) = std::fs::read_to_string("/proc/stat") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("btime ") {
                return rest.trim().parse().unwrap_or(0);
            }
        }
    }
    0
}

fn read_os_field(field: &str) -> String {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{}=", field)) {
                return rest.trim_matches('"').to_string();
            }
        }
    }
    String::new()
}
