use std::fs;

/// Get system load averages from /proc/loadavg
pub fn get_load() -> anyhow::Result<(f64, f64, f64)> {
    let content = fs::read_to_string("/proc/loadavg")?;
    let parts: Vec<&str> = content.split_whitespace().collect();

    if parts.len() < 3 {
        anyhow::bail!("unexpected /proc/loadavg format");
    }

    let load1: f64 = parts[0].parse()?;
    let load5: f64 = parts[1].parse()?;
    let load15: f64 = parts[2].parse()?;

    Ok((load1, load5, load15))
}
