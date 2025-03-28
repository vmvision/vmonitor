use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub endpoints: Vec<Endpoint>,
    #[serde(default = "default_connection")]
    pub connection: ConnectionConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Endpoint {
    pub name: String,
    pub server: String,
    pub secret: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "Option::default")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Copy)]
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

fn default_connection() -> ConnectionConfig {
    ConnectionConfig {
        base_delay: default_base_delay(),
        max_delay: default_max_delay(),
        max_retries: default_max_retries(),
    }
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
