use futures_util::{SinkExt, StreamExt};
use sysinfo::{Disks, Networks, System};
use tokio::signal;
use tokio::time::{interval, sleep, Duration};
use tokio_tungstenite::tungstenite::{Bytes, Message};
use tracing::{error, info, warn};

use crate::api;
use crate::config::{AppConfig, EndpointConfig};
use crate::monitor;

pub struct App {
    config: AppConfig,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) {
        // Listen for exit signals (Ctrl+C)
        let shutdown_signal = async {
            signal::ctrl_c()
                .await
                .expect("Failed to listen for shutdown signal");
            info!("Received shutdown signal");
        };

        // Create tasks for each enabled endpoint
        let endpoint_tasks: Vec<_> = self
            .config
            .endpoints
            .iter()
            .filter(|endpoint| endpoint.enabled)
            .map(|endpoint| self.run_endpoint(endpoint.clone()))
            .collect();

        // Run all endpoint tasks and exit listening simultaneously
        tokio::select! {
            _ = shutdown_signal => {
                info!("Shutting down...");
            }
            _ = futures::future::join_all(endpoint_tasks) => {
                warn!("All endpoint tasks completed");
            }
        }
    }

    async fn run_endpoint(&self, endpoint: EndpointConfig) {
        let mut system = System::new();
        let mut networks = Networks::new();
        let mut disks = Disks::new();

        let mut retry_count = 0;
        loop {
            info!(endpoint = %endpoint.name, "Connecting to WebSocket...");
            let socket = match api::connect_websocket(
                &endpoint.websocket_url,
                &endpoint.auth_secret,
                &api::ConnectionConfig {
                    base_delay: self.config.connection.base_delay,
                    max_delay: self.config.connection.max_delay,
                    max_retries: self.config.connection.max_retries,
                },
            )
            .await
            {
                Some(socket) => socket,
                None => {
                    error!(endpoint = %endpoint.name, "Failed to connect to WebSocket, retrying...");
                    retry_count += 1;
                    let delay = self.config.connection.base_delay
                        * 2u64.pow(retry_count.min(16) as u32 - 1);
                    let delay = delay.min(self.config.connection.max_delay);
                    sleep(Duration::from_secs(delay)).await;
                    continue;
                }
            };

            info!(endpoint = %endpoint.name, "WebSocket connection established");

            let (mut write, mut read) = socket.split();
            let mut interval = interval(Duration::from_secs(self.config.interval));

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
            let delay = self.config.connection.base_delay * 2u64.pow(retry_count.min(16) as u32 - 1);
            let delay = delay.min(self.config.connection.max_delay);
            sleep(Duration::from_secs(delay)).await;
        }
    }
} 