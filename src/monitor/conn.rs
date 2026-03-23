use std::fs;

/// Count TCP and UDP connections from /proc/net/{tcp,tcp6,udp,udp6}
pub fn get_connections() -> anyhow::Result<(u64, u64)> {
    let mut tcp_count: u64 = 0;
    let mut udp_count: u64 = 0;

    // Count lines in each file (skip header)
    for path in &["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(content) = fs::read_to_string(path) {
            tcp_count += content.lines().skip(1).count() as u64;
        }
    }

    for path in &["/proc/net/udp", "/proc/net/udp6"] {
        if let Ok(content) = fs::read_to_string(path) {
            udp_count += content.lines().skip(1).count() as u64;
        }
    }

    Ok((tcp_count, udp_count))
}
