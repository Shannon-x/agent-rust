use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

// Cached CPU stat for non-blocking usage calculation
static PREV_TOTAL: AtomicU64 = AtomicU64::new(0);
static PREV_IDLE: AtomicU64 = AtomicU64::new(0);
static CACHED_USAGE: AtomicU64 = AtomicU64::new(0);

/// Get CPU model information for host info
pub fn get_host_info() -> anyhow::Result<Vec<String>> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")?;
    let mut model_counts: HashMap<String, u32> = HashMap::new();

    let mut current_model = String::new();
    for line in cpuinfo.lines() {
        if let Some(model) = line.strip_prefix("model name\t: ") {
            current_model = model.trim().to_string();
        }
        if (line.starts_with("cpu cores\t:") || line.is_empty()) && !current_model.is_empty() {
            *model_counts.entry(current_model.clone()).or_insert(0) += 1;
        }
    }

    // ARM fallback
    if model_counts.is_empty() {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_all();
        for cpu in sys.cpus() {
            *model_counts.entry(cpu.brand().to_string()).or_insert(0) += 1;
        }
    }

    let result: Vec<String> = model_counts
        .into_iter()
        .map(|(model, count)| format!("{} {} Physical Core", model, count))
        .collect();

    Ok(result)
}

/// Non-blocking CPU usage — reads /proc/stat once per call, compares with cached previous.
/// PERF: Eliminates the 200ms sleep by caching between report cycles.
/// The report_delay (1-4s) provides natural measurement interval.
pub fn get_usage_cached() -> f64 {
    let stat = match read_cpu_stat() {
        Ok(s) => s,
        Err(_) => return f64::from_bits(CACHED_USAGE.load(Ordering::Relaxed)),
    };

    let prev_total = PREV_TOTAL.load(Ordering::Relaxed);
    let prev_idle = PREV_IDLE.load(Ordering::Relaxed);

    PREV_TOTAL.store(stat.total, Ordering::Relaxed);
    PREV_IDLE.store(stat.idle, Ordering::Relaxed);

    // First call — no previous data yet
    if prev_total == 0 {
        return 0.0;
    }

    let total_diff = stat.total.saturating_sub(prev_total) as f64;
    let idle_diff = stat.idle.saturating_sub(prev_idle) as f64;

    if total_diff == 0.0 {
        return f64::from_bits(CACHED_USAGE.load(Ordering::Relaxed));
    }

    let usage = ((total_diff - idle_diff) / total_diff) * 100.0;
    CACHED_USAGE.store(usage.to_bits(), Ordering::Relaxed);
    usage
}

struct CpuStat {
    total: u64,
    idle: u64,
}

fn read_cpu_stat() -> anyhow::Result<CpuStat> {
    let content = fs::read_to_string("/proc/stat")?;
    let first_line = content
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty /proc/stat"))?;

    let parts: Vec<u64> = first_line
        .split_whitespace()
        .skip(1) // skip "cpu"
        .filter_map(|s| s.parse().ok())
        .collect();

    if parts.len() < 4 {
        anyhow::bail!("unexpected /proc/stat format");
    }

    let total: u64 = parts.iter().sum();
    let idle = parts[3] + if parts.len() > 4 { parts[4] } else { 0 }; // idle + iowait

    Ok(CpuStat { total, idle })
}
