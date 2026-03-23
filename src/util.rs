use md5::{Digest, Md5};
use std::net::IpAddr;

#[allow(dead_code)]
pub const USER_AGENT: &str = "nezha-agent/1.0";

pub const DNS_SERVERS_V4: &[&str] = &["8.8.8.8:53", "8.8.4.4:53", "1.1.1.1:53", "1.0.0.1:53"];
pub const DNS_SERVERS_V6: &[&str] = &[
    "[2001:4860:4860::8888]:53",
    "[2001:4860:4860::8844]:53",
    "[2606:4700:4700::1111]:53",
    "[2606:4700:4700::1001]:53",
];

/// Compute MD5 hex digest of a string
#[allow(dead_code)]
pub fn md5_hex(input: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Subtract with underflow check (returns 0 on underflow)
#[allow(dead_code)]
pub fn sub_checked(a: u64, b: u64) -> u64 {
    a.saturating_sub(b)
}

/// Resolve hostname to IP, returning the first result
pub async fn lookup_ip(host_or_ip: &str) -> anyhow::Result<String> {
    // If it's already an IP, return directly
    if host_or_ip.parse::<IpAddr>().is_ok() {
        return Ok(host_or_ip.to_string());
    }

    // Use tokio's built-in DNS resolution
    let addrs: Vec<_> = tokio::net::lookup_host(format!("{}:0", host_or_ip))
        .await?
        .collect();

    if addrs.is_empty() {
        anyhow::bail!("无法解析 {}", host_or_ip);
    }

    Ok(addrs[0].ip().to_string())
}

/// Rotate start index based on current time
#[allow(dead_code)]
pub fn rotate_index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    (now as usize) % len
}
