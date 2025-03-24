use futures_util::{SinkExt, StreamExt};
use sysinfo::{Disks, Networks, System};
use tokio::signal;
use tokio::time::{interval, sleep, Duration};
use tokio_tungstenite::tungstenite::{Bytes, Message};
use tracing::{error, info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::api;
use crate::config::AppConfig;
use crate::monitor;

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
        self.setup_endpoints().await;

        // Run all endpoint tasks and exit listening simultaneously
        tokio::select! {
            _ = shutdown_signal => {
                info!("Shutting down...");
            }
            _ = self.monitor_config_changes() => {
                warn!("Config monitoring completed");
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
            let config = self.config.clone();
            let tasks = self.endpoint_tasks.clone();
            
            let task = tokio::spawn(async move {
                let mut system = System::new();
                let mut networks = Networks::new();
                let mut disks = Disks::new();

                let mut retry_count = 0;
                loop {
                    info!(endpoint = %endpoint.name, "Connecting to WebSocket...");
                    let socket = match api::connect_websocket(
                        &endpoint.server,
                        &endpoint.secret,
                        &api::ConnectionConfig {
                            base_delay: config.read().await.connection.base_delay,
                            max_delay: config.read().await.connection.max_delay,
                            max_retries: config.read().await.connection.max_retries,
                        },
                    )
                    .await
                    {
                        Some(socket) => socket,
                        None => {
                            error!(endpoint = %endpoint.name, "Failed to connect to WebSocket, retrying...");
                            retry_count += 1;
                            let delay = config.read().await.connection.base_delay
                                * 2u64.pow(retry_count.min(16) as u32 - 1);
                            let delay = delay.min(config.read().await.connection.max_delay);
                            sleep(Duration::from_secs(delay)).await;
                            continue;
                        }
                    };

                    info!(endpoint = %endpoint.name, "WebSocket connection established");

                    let (mut write, mut read) = socket.split();
                    let mut interval = interval(Duration::from_secs(config.read().await.metrics_interval));

                    loop {
                        tokio::select! {
                            _ = interval.tick() => {
                                monitor::send_metrics(&mut write, &mut system, &mut networks, &mut disks).await;
                            }
                            Some(msg) = read.next() => {
                                match msg {
                                    Ok(Message::Text(text)) => {
                                        info!(endpoint = %endpoint.name, message = %text, "Received WebSocket message");
                                        match serde_json::from_str::<serde_json::Value>(&text) {
                                            Ok(json) => info!(endpoint = %endpoint.name, json = ?json, "Parsed WebSocket message"),
                                            Err(e) => warn!(endpoint = %endpoint.name, error = %e, "Failed to parse WebSocket message as JSON"),
                                        }
                                    }
                                    Ok(Message::Binary(binary)) => {
                                        info!(endpoint = %endpoint.name, binary = ?binary, "Received binary message");
                                        match rmp_serde::from_slice::<api::Message<serde_json::Value>>(&binary) {
                                            Ok(api_msg) => {
                                                if api_msg.r#type == "get_info" {
                                                    let vm_info = monitor::collect_vm_info(&mut system, &mut disks);
                                                    info!(endpoint = %endpoint.name, vm_info = ?vm_info, "Sending VM info response");
                                                    let response = api::Message {
                                                        r#type: "vm_info".to_string(),
                                                        data: vm_info,
                                                    };
                                                    if let Ok(msgpack) = rmp_serde::to_vec_named(&response) {
                                                        if let Err(e) = write.send(Message::Binary(Bytes::from(msgpack))).await {
                                                            warn!(endpoint = %endpoint.name, error = %e, "Failed to send VM info response");
                                                        }
                                                        info!(endpoint = %endpoint.name, "Sent VM info response");
                                                    }
                                                }
                                                info!(endpoint = %endpoint.name, message = ?api_msg, "Parsed MessagePack message");
                                            }
                                            Err(e) => warn!(endpoint = %endpoint.name, error = %e, "Failed to parse as api::Message"),
                                        }
                                    }
                                    Ok(Message::Ping(ping)) => {
                                        if let Err(e) = write.send(Message::Pong(ping)).await {
                                            error!(endpoint = %endpoint.name, error = %e, "Failed to send pong response");
                                        }
                                    }
                                    Err(e) => {
                                        error!(endpoint = %endpoint.name, error = %e, "WebSocket read error, attempting reconnect");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    warn!(endpoint = %endpoint.name, "WebSocket connection lost, reconnecting...");
                    retry_count += 1;
                    let delay = config.read().await.connection.base_delay * 2u64.pow(retry_count.min(16) as u32 - 1);
                    let delay = delay.min(config.read().await.connection.max_delay);
                    sleep(Duration::from_secs(delay)).await;
                }
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