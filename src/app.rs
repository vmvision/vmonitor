use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::monitor::Monitor;

pub struct App {
    config: Arc<RwLock<AppConfig>>,
    endpoint_tasks: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            endpoint_tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn run(&self) {
        // Listen for exit signals (Ctrl+C)
        let shutdown_signal = async {
            signal::ctrl_c()
                .await
                .expect("Failed to listen for shutdown signal");
            info!("Received shutdown signal");
        };

        // Initial endpoint setup
        // Run endpoint setup and monitoring tasks simultaneously
        tokio::select! {
            _ = self.setup_endpoints() => {
                warn!("Endpoint setup completed");
                                let mut tasks = self.endpoint_tasks.write().await;
                for task in tasks.iter_mut() {
                    task.abort();
                }
                tasks.clear();
            }
            _ = shutdown_signal => {
                info!("Shutting down...");
                // Abort all running tasks
                let mut tasks = self.endpoint_tasks.write().await;
                for task in tasks.iter_mut() {
                    task.abort();
                }
                tasks.clear();
            }
            _ = self.monitor_config_changes() => {
                warn!("Config monitoring completed");
                // Abort all running tasks
                let mut tasks = self.endpoint_tasks.write().await;
                for task in tasks.iter_mut() {
                    task.abort();
                }
                tasks.clear();
            }
        }
    }

    async fn setup_endpoints(&self) {
        let config = self.config.read().await;
        let mut tasks = self.endpoint_tasks.write().await;

        // Clear existing tasks
        for task in tasks.iter_mut() {
            task.abort();
        }
        tasks.clear();

        // Create new tasks for enabled endpoints
        for endpoint in config.endpoints.iter().filter(|e| e.enabled) {
            let endpoint = endpoint.clone();
            let tasks = self.endpoint_tasks.clone();
            let task = tokio::spawn(async move {
                let monitor = Monitor::new(endpoint);
                monitor.run().await;
            });
            let mut tasks_lock = tasks.write().await;
            tasks_lock.push(task);
        }
    }

    async fn monitor_config_changes(&self) {
        let mut interval = interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            if let Ok(new_config) = AppConfig::from_file("config.toml") {
                let current_config = self.config.read().await;
                if new_config != *current_config {
                    info!("Configuration changed, reloading endpoints...");
                    let mut config_lock = self.config.write().await;
                    *config_lock = new_config;
                    self.setup_endpoints().await;
                }
            }
        }
    }
}
