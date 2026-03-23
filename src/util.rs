#[allow(dead_code)]
pub const USER_AGENT: &str = "nezha-agent/1.0";

#[allow(dead_code)]
pub const DNS_SERVERS_V4: &[&str] = &["8.8.8.8:53", "8.8.4.4:53", "1.1.1.1:53", "1.0.0.1:53"];
#[allow(dead_code)]
pub const DNS_SERVERS_V6: &[&str] = &[
    "[2001:4860:4860::8888]:53",
    "[2001:4860:4860::8844]:53",
    "[2606:4700:4700::1111]:53",
    "[2606:4700:4700::1001]:53",
];

/// Resolve hostname to IP, returning the first result
pub async fn lookup_ip(host_or_ip: &str) -> anyhow::Result<String> {
    use std::net::IpAddr;
    if host_or_ip.parse::<IpAddr>().is_ok() {
        return Ok(host_or_ip.to_string());
    }
    let addrs: Vec<_> = tokio::net::lookup_host(format!("{}:0", host_or_ip))
        .await?
        .collect();
    if addrs.is_empty() {
        anyhow::bail!("无法解析 {}", host_or_ip);
    }
    Ok(addrs[0].ip().to_string())
}
