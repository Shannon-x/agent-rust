use aho_corasick::AhoCorasick;
use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;

const EXCLUDE_INTERFACES: &[&str] = &[
    "lo",
    "tun",
    "docker",
    "veth",
    "br-",
    "vmbr",
    "vnet",
    "kube",
    "Meta",
    "tailscale",
    "fw",
    "tap",
];

static INTERFACE_MATCHER: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(EXCLUDE_INTERFACES).unwrap());

/// Get network traffic from /proc/net/dev
pub fn get_traffic(allowlist: &HashMap<String, bool>) -> anyhow::Result<(u64, u64)> {
    let content = fs::read_to_string("/proc/net/dev")?;
    let mut in_transfer: u64 = 0;
    let mut out_transfer: u64 = 0;

    for line in content.lines().skip(2) {
        // Skip header lines
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        let iface = parts[0].trim_end_matches(':');

        // Apply filtering
        if INTERFACE_MATCHER.is_match(iface.as_bytes())
            && !allowlist.get(iface).copied().unwrap_or(false)
        {
            continue;
        }
        if !allowlist.is_empty() && !allowlist.get(iface).copied().unwrap_or(false) {
            continue;
        }

        let bytes_recv: u64 = parts[1].parse().unwrap_or(0);
        let bytes_sent: u64 = parts[9].parse().unwrap_or(0);

        in_transfer += bytes_recv;
        out_transfer += bytes_sent;
    }

    Ok((in_transfer, out_transfer))
}
