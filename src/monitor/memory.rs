use std::fs;

/// Get memory stats from /proc/meminfo
pub fn get_memory_info() -> anyhow::Result<MemoryInfo> {
    let content = fs::read_to_string("/proc/meminfo")?;
    let mut info = MemoryInfo::default();

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let value: u64 = parts[1].parse().unwrap_or(0) * 1024; // Convert kB to bytes
        match parts[0] {
            "MemTotal:" => info.total = value,
            "MemAvailable:" => info.available = value,
            "SwapTotal:" => info.swap_total = value,
            "SwapFree:" => info.swap_free = value,
            _ => {}
        }
    }

    info.used = info.total.saturating_sub(info.available);
    info.swap_used = info.swap_total.saturating_sub(info.swap_free);

    Ok(info)
}

#[derive(Default, Debug)]
pub struct MemoryInfo {
    pub total: u64,
    pub available: u64,
    pub used: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub swap_used: u64,
}
