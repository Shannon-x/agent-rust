use crate::proto;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tracing::info;

const CF_ENDPOINTS: &[&str] = &[
    "https://blog.cloudflare.com/cdn-cgi/trace",
    "https://developers.cloudflare.com/cdn-cgi/trace",
    "https://hostinger.com/cdn-cgi/trace",
    "https://ahrefs.com/cdn-cgi/trace",
];

const USER_AGENT: &str = "nezha-agent/1.0";

static GEO_QUERY_IP: Mutex<Option<String>> = Mutex::new(None);
pub static GEO_QUERY_IP_CHANGED: AtomicBool = AtomicBool::new(true);
static CACHED_COUNTRY_CODE: Mutex<Option<String>> = Mutex::new(None);
static RETRY_TIMES: AtomicI32 = AtomicI32::new(0);
static FAILED_STARTED_AT: Mutex<Option<Instant>> = Mutex::new(None);
static LATEST_RETRY_AT: Mutex<Option<Instant>> = Mutex::new(None);

#[allow(dead_code)]
pub fn get_cached_country_code() -> String {
    CACHED_COUNTRY_CODE
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default()
}

pub fn set_cached_country_code(code: &str) {
    *CACHED_COUNTRY_CODE.lock().unwrap() = Some(code.to_string());
}

/// Fetch public IP addresses and return GeoIP protobuf
pub async fn fetch_ip(
    use_ipv6_country_code: bool,
    custom_endpoints: &[String],
) -> Option<proto::GeoIp> {
    info!("正在更新本地缓存IP信息");

    let retry_count = RETRY_TIMES.load(Ordering::Relaxed);
    if retry_count > 2 {
        let latest = LATEST_RETRY_AT.lock().unwrap();
        let failed = FAILED_STARTED_AT.lock().unwrap();
        if let (Some(latest_at), Some(failed_at)) = (latest.as_ref(), failed.as_ref()) {
            let backoff = latest_at.duration_since(*failed_at) * 2;
            if Instant::now() < *latest_at + backoff {
                info!("IP地址获取失败次数过多，fallback到agent连接IP");
                return Some(proto::GeoIp {
                    use6: false,
                    ip: Some(proto::Ip {
                        ipv4: String::new(),
                        ipv6: String::new(),
                    }),
                    country_code: String::new(),
                    dashboard_boot_time: 0,
                });
            }
        }
    }

    let endpoints: Vec<String> = if custom_endpoints.is_empty() {
        CF_ENDPOINTS.iter().map(|s| (*s).to_string()).collect()
    } else {
        custom_endpoints.to_vec()
    };

    let endpoints_clone = endpoints.clone();
    let (ipv4_result, ipv6_result) = tokio::join!(
        fetch_single_ip(&endpoints, false),
        fetch_single_ip(&endpoints_clone, true),
    );

    let ipv4 = ipv4_result.unwrap_or_default();
    let ipv6 = ipv6_result.unwrap_or_default();

    let query_ip = if !ipv6.is_empty() && (use_ipv6_country_code || ipv4.is_empty()) {
        ipv6.clone()
    } else if !ipv4.is_empty() {
        ipv4.clone()
    } else {
        String::new()
    };

    if !query_ip.is_empty() {
        let mut geo = GEO_QUERY_IP.lock().unwrap();
        let changed =
            geo.as_deref() != Some(&query_ip) || GEO_QUERY_IP_CHANGED.load(Ordering::Relaxed);
        GEO_QUERY_IP_CHANGED.store(changed, Ordering::Relaxed);
        *geo = Some(query_ip);

        RETRY_TIMES.store(0, Ordering::Relaxed);
        return Some(proto::GeoIp {
            use6: use_ipv6_country_code,
            ip: Some(proto::Ip { ipv4, ipv6 }),
            country_code: String::new(),
            dashboard_boot_time: 0,
        });
    }

    let now = Instant::now();
    let count = RETRY_TIMES.fetch_add(1, Ordering::Relaxed) + 1;
    *LATEST_RETRY_AT.lock().unwrap() = Some(now);
    if count == 1 {
        *FAILED_STARTED_AT.lock().unwrap() = Some(now);
    }

    None
}

async fn fetch_single_ip(servers: &[String], is_v6: bool) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .connect_timeout(std::time::Duration::from_secs(5))
        .user_agent(USER_AGENT)
        .build()
        .ok()?;

    for server in servers {
        match client.get(server).send().await {
            Ok(resp) => {
                if let Ok(body) = resp.text().await {
                    let ip = parse_ip_from_response(&body);
                    if let Some(parsed) = validate_ip(&ip, is_v6) {
                        return Some(parsed);
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("no route to host") {
                    return None;
                }
                continue;
            }
        }
    }
    None
}

fn parse_ip_from_response(body: &str) -> String {
    if !body.contains("ip=") {
        return body.trim().replace('\n', "");
    }
    for line in body.lines() {
        if let Some(ip) = line.strip_prefix("ip=") {
            return ip.to_string();
        }
    }
    String::new()
}

fn validate_ip(ip_str: &str, is_v6: bool) -> Option<String> {
    let parsed: IpAddr = ip_str.parse().ok()?;
    match (is_v6, parsed) {
        (true, IpAddr::V6(_)) | (false, IpAddr::V4(_)) => Some(ip_str.to_string()),
        _ => None,
    }
}
