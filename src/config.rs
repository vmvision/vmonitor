use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    // Global settings
    pub metrics_interval: u64,
    pub ip_report_interval: u64,
    pub connection: ConnectionConfig,
    // Endpoints
    pub endpoints: Vec<EndpointConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EndpointConfig {
    pub name: String,
    pub server: String,
    pub secret: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "Option::default")]
    pub metrics_interval: Option<u64>,
    #[serde(default = "Option::default")]
    pub ip_report_interval: Option<u64>,
    #[serde(default = "Option::default")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ConnectionConfig {
    #[serde(default = "default_base_delay")]
    pub base_delay: u64,
    #[serde(default = "default_max_delay")]
    pub max_delay: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: i32,
}

fn default_base_delay() -> u64 {
    1
}

fn default_max_delay() -> u64 {
    60
}

fn default_max_retries() -> i32 {
    -1
}

fn default_enabled() -> bool {
    true
}

impl AppConfig {
    pub fn from_file(path: &str) -> Result<Self, config::ConfigError> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name(path))
            .build()?;
        cfg.try_deserialize()
    }

    pub fn save_to_file(&self, path: &str) -> Result<(), std::io::Error> {
        let toml = toml::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to serialize config: {}", e),
            )
        })?;
        std::fs::write(path, toml)
    }
}
