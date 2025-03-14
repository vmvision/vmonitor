use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub websocket_url: String,
    pub auth_secret: String,
    pub interval: u64,
    pub connection: ConnectionConfig,
}

#[derive(Debug, Serialize, Deserialize)]
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

impl AppConfig {
    pub fn from_file(path: &str) -> Result<Self, config::ConfigError> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name(path))
            .build()?;
        cfg.try_deserialize()
    }
}
