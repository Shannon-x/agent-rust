use std::collections::HashMap;
use std::fs;
use std::process::Command;
use tracing::warn;

const EXPECTED_FS_TYPES: &[&str] = &[
    "apfs", "ext4", "ext3", "ext2", "f2fs", "reiserfs", "jfs", "bcachefs", "btrfs",
    "fuseblk", "zfs", "simfs", "ntfs", "fat32", "exfat", "xfs", "fuse.rclone",
];

/// Get total disk space
pub fn get_total(allowlist: &[String]) -> u64 {
    let devices = match get_devices(allowlist) {
        Ok(d) => d,
        Err(_) => return 0,
    };

    let mut total: u64 = 0;
    for mount_path in devices.values() {
        if let Ok(stat) = nix::sys::statvfs::statvfs(mount_path.as_str()) {
            total += stat.blocks() as u64 * stat.fragment_size() as u64;
        }
    }

    // Fallback for OpenVZ-like systems
    if total == 0 {
        total = df_fallback_total();
    }

    total
}

/// Get used disk space
pub fn get_used(allowlist: &[String]) -> u64 {
    let devices = match get_devices(allowlist) {
        Ok(d) => d,
        Err(_) => return 0,
    };

    let mut used: u64 = 0;
    for mount_path in devices.values() {
        if let Ok(stat) = nix::sys::statvfs::statvfs(mount_path.as_str()) {
            let total = stat.blocks() as u64 * stat.fragment_size() as u64;
            let avail = stat.blocks_available() as u64 * stat.fragment_size() as u64;
            used += total.saturating_sub(avail);
        }
    }

    if used == 0 {
        used = df_fallback_used();
    }

    used
}

fn get_devices(allowlist: &[String]) -> anyhow::Result<HashMap<String, String>> {
    let mut devices = HashMap::new();

    // Use allowlist if configured
    if !allowlist.is_empty() {
        for (i, v) in allowlist.iter().enumerate() {
            devices.insert(i.to_string(), v.clone());
        }
        return Ok(devices);
    }

    // Parse /proc/mounts for partition discovery
    let mounts = fs::read_to_string("/proc/mounts")?;
    for line in mounts.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let device = parts[0];
        let mount_point = parts[1];
        let fs_type = parts[2].to_lowercase();

        // Filter by expected filesystem types
        if EXPECTED_FS_TYPES.iter().any(|t| fs_type.contains(t))
            && !mount_point.contains("/var/lib/kubelet")
            && !devices.contains_key(device)
        {
            devices.insert(device.to_string(), mount_point.to_string());
        }
    }

    Ok(devices)
}

fn df_fallback_total() -> u64 {
    if let Ok(output) = Command::new("df").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let info: Vec<&str> = line.split_whitespace().collect();
            if info.len() == 6 && info[5] == "/" {
                return info[1].parse::<u64>().unwrap_or(0) * 1024;
            }
        }
    }
    0
}

fn df_fallback_used() -> u64 {
    if let Ok(output) = Command::new("df").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let info: Vec<&str> = line.split_whitespace().collect();
            if info.len() == 6 && info[5] == "/" {
                return info[2].parse::<u64>().unwrap_or(0) * 1024;
            }
        }
    }
    0
}
