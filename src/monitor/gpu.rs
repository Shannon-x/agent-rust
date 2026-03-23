use std::path::Path;
use std::process::Command;
use tracing::warn;

/// GPU vendor detection
enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    None,
}

fn detect_vendor() -> GpuVendor {
    if Path::new("/usr/bin/nvidia-smi").exists() {
        return GpuVendor::Nvidia;
    }
    if Path::new("/opt/rocm/bin/rocm-smi").exists() {
        return GpuVendor::Amd;
    }
    if Path::new("/usr/bin/intel_gpu_top").exists() {
        return GpuVendor::Intel;
    }
    GpuVendor::None
}

/// Get GPU model information
pub fn get_host_info() -> anyhow::Result<Vec<String>> {
    match detect_vendor() {
        GpuVendor::Nvidia => nvidia_model(),
        GpuVendor::Amd => amd_model(),
        GpuVendor::Intel => intel_model(),
        GpuVendor::None => Ok(Vec::new()),
    }
}

/// Get GPU usage percentages
pub fn get_usage() -> anyhow::Result<Vec<f64>> {
    match detect_vendor() {
        GpuVendor::Nvidia => nvidia_usage(),
        GpuVendor::Amd => amd_usage(),
        GpuVendor::Intel => intel_usage(),
        GpuVendor::None => Ok(Vec::new()),
    }
}

fn nvidia_model() -> anyhow::Result<Vec<String>> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("nvidia-smi failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

fn nvidia_usage() -> anyhow::Result<Vec<f64>> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("nvidia-smi failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|s| s.trim().parse::<f64>().ok())
        .collect())
}

fn amd_model() -> anyhow::Result<Vec<String>> {
    let output = Command::new("/opt/rocm/bin/rocm-smi")
        .args(["--showproductname"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("rocm-smi failed");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut models = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.contains("Card series:") || trimmed.contains("Card model:") {
            if let Some(name) = trimmed.split(':').nth(1) {
                models.push(name.trim().to_string());
            }
        }
    }
    Ok(models)
}

fn amd_usage() -> anyhow::Result<Vec<f64>> {
    let output = Command::new("/opt/rocm/bin/rocm-smi")
        .args(["--showuse"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("rocm-smi failed");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut usages = Vec::new();
    for line in stdout.lines() {
        if line.contains("GPU use (%)") || line.contains("GPU Activity") {
            if let Some(val) = line.split_whitespace().last() {
                if let Ok(v) = val.trim_end_matches('%').parse::<f64>() {
                    usages.push(v);
                }
            }
        }
    }
    Ok(usages)
}

fn intel_model() -> anyhow::Result<Vec<String>> {
    // Intel iGPU model detection from /sys/class/drm
    let drm_path = Path::new("/sys/class/drm");
    if !drm_path.exists() {
        return Ok(Vec::new());
    }
    // Simple detection
    Ok(vec!["Intel Integrated Graphics".to_string()])
}

fn intel_usage() -> anyhow::Result<Vec<f64>> {
    let output = Command::new("intel_gpu_top")
        .args(["-J", "-s", "500"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("intel_gpu_top failed");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse JSON output for busy percentage
    if let Some(busy_line) = stdout.lines().find(|l| l.contains("busy")) {
        if let Some(val) = busy_line.split(':').nth(1) {
            if let Ok(v) = val.trim().trim_matches(',').parse::<f64>() {
                return Ok(vec![v]);
            }
        }
    }
    Ok(Vec::new())
}
