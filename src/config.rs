use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub debug: bool,

    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub uuid: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hard_drive_partition_allowlist: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub nic_allowlist: HashMap<String, bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<String>,

    #[serde(default)]
    pub gpu: bool,
    #[serde(default)]
    pub temperature: bool,
    #[serde(default)]
    pub skip_connection_count: bool,
    #[serde(default)]
    pub skip_procs_count: bool,
    #[serde(default)]
    pub disable_auto_update: bool,
    #[serde(default)]
    pub disable_force_update: bool,
    #[serde(default)]
    pub disable_command_execute: bool,
    #[serde(default)]
    pub report_delay: u32,
    #[serde(default)]
    pub tls: bool,
    #[serde(default)]
    pub insecure_tls: bool,
    #[serde(default)]
    pub use_ipv6_country_code: bool,
    #[serde(default)]
    pub use_gitee_to_upgrade: bool,
    #[serde(default)]
    pub use_atomgit_to_upgrade: bool,
    #[serde(default)]
    pub disable_nat: bool,
    #[serde(default)]
    pub disable_send_query: bool,
    #[serde(default)]
    pub ip_report_period: u32,
    #[serde(default)]
    pub self_update_period: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_ip_api: Vec<String>,

    #[serde(skip)]
    pub file_path: PathBuf,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            debug: false,
            server: String::new(),
            client_secret: String::new(),
            uuid: String::new(),
            hard_drive_partition_allowlist: Vec::new(),
            nic_allowlist: HashMap::new(),
            dns: Vec::new(),
            gpu: false,
            temperature: false,
            skip_connection_count: false,
            skip_procs_count: false,
            disable_auto_update: false,
            disable_force_update: false,
            disable_command_execute: false,
            report_delay: 3,
            tls: false,
            insecure_tls: false,
            use_ipv6_country_code: false,
            use_gitee_to_upgrade: false,
            use_atomgit_to_upgrade: false,
            disable_nat: false,
            disable_send_query: false,
            ip_report_period: 1800,
            self_update_period: 0,
            custom_ip_api: Vec::new(),
            file_path: PathBuf::new(),
        }
    }
}

impl AgentConfig {
    /// Read configuration from file and environment variables
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let mut config = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            serde_yaml::from_str::<AgentConfig>(&content).unwrap_or_default()
        } else {
            AgentConfig::default()
        };

        config.file_path = path.to_path_buf();

        // Override with NZ_ environment variables
        config.load_env_overrides();

        // Generate UUID if empty
        if config.uuid.is_empty() {
            config.uuid = Uuid::new_v4().to_string();
            config.save()?;
        }

        config.validate(false)?;

        // Save if file didn't exist
        if !path.exists() {
            config.save()?;
        }

        Ok(config)
    }

    /// Load environment variable overrides (NZ_ prefix)
    fn load_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("NZ_SERVER") {
            self.server = v;
        }
        if let Ok(v) = std::env::var("NZ_CLIENT_SECRET") {
            self.client_secret = v;
        }
        if let Ok(v) = std::env::var("NZ_UUID") {
            self.uuid = v;
        }
        if let Ok(v) = std::env::var("NZ_DEBUG") {
            self.debug = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_TLS") {
            self.tls = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_INSECURE_TLS") {
            self.insecure_tls = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_GPU") {
            self.gpu = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_TEMPERATURE") {
            self.temperature = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_DISABLE_AUTO_UPDATE") {
            self.disable_auto_update = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_DISABLE_FORCE_UPDATE") {
            self.disable_force_update = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_DISABLE_COMMAND_EXECUTE") {
            self.disable_command_execute = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_DISABLE_NAT") {
            self.disable_nat = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_DISABLE_SEND_QUERY") {
            self.disable_send_query = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_USE_IPV6_COUNTRY_CODE") {
            self.use_ipv6_country_code = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_REPORT_DELAY") {
            if let Ok(n) = v.parse() {
                self.report_delay = n;
            }
        }
        if let Ok(v) = std::env::var("NZ_IP_REPORT_PERIOD") {
            if let Ok(n) = v.parse() {
                self.ip_report_period = n;
            }
        }
        if let Ok(v) = std::env::var("NZ_SKIP_CONNECTION_COUNT") {
            self.skip_connection_count = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("NZ_SKIP_PROCS_COUNT") {
            self.skip_procs_count = v == "true" || v == "1";
        }
    }

    /// Validate configuration
    pub fn validate(&mut self, is_remote_edit: bool) -> anyhow::Result<()> {
        if self.report_delay == 0 {
            self.report_delay = 3;
        }
        if self.ip_report_period == 0 {
            self.ip_report_period = 1800;
        } else if self.ip_report_period < 30 {
            self.ip_report_period = 30;
        }
        if self.report_delay < 1 || self.report_delay > 4 {
            anyhow::bail!("report-delay ranges from 1-4");
        }
        if !is_remote_edit {
            if self.server.is_empty() {
                anyhow::bail!("server address should not be empty");
            }
            if self.client_secret.is_empty() {
                anyhow::bail!("client_secret must be specified");
            }
            if Uuid::parse_str(&self.uuid).is_err() {
                anyhow::bail!("invalid UUID format: {}", self.uuid);
            }
        }
        Ok(())
    }

    /// Save configuration to file
    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(dir) = self.file_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let data = serde_yaml::to_string(self)?;
        std::fs::write(&self.file_path, data)?;
        Ok(())
    }
}
