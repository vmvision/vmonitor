mod api;
mod config;
mod monitor;

use futures_util::{SinkExt, StreamExt};
use std::env;
use sysinfo::{Disks, Networks, System};
use tokio::signal;
use tokio::time::{interval, sleep, Duration};
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt().with_env_filter("info").init();

    let config_path =
        env::var("VMONITOR_CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
    info!(config_path = %config_path, "Starting application");

    // Load configuration from config file
    let config = match config::AppConfig::from_file(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!(error = %e, "Failed to load config");
            std::process::exit(1);
        }
    };
    info!("Configuration loaded");

    // Listen for exit signals (Ctrl+C)
    let shutdown_signal = async {
        signal::ctrl_c()
            .await
            .expect("Failed to listen for shutdown signal");
        info!("Received shutdown signal");
    };

    // Maintain WebSocket connection
    let maintain_connection = async {
        let mut system = System::new();
        let mut networks = Networks::new();
        let mut disks = Disks::new();

        let mut retry_count = 0;
        loop {
            info!("Connecting to WebSocket...");
            let socket = match api::connect_websocket(
                &config.websocket_url,
                &config.auth_secret,
                &api::ConnectionConfig {
                    base_delay: config.connection.base_delay,
                    max_delay: config.connection.max_delay,
                    max_retries: config.connection.max_retries,
                },
            )
            .await
            {
                Some(socket) => socket,
                None => {
                    error!("Failed to connect to WebSocket, retrying...");
                    retry_count += 1;
                    let delay =
                        config.connection.base_delay * 2u64.pow(retry_count.min(16) as u32 - 1);
                    let delay = delay.min(config.connection.max_delay);
                    sleep(Duration::from_secs(delay)).await;
                    continue;
                }
            };

            info!("WebSocket connection established");

            let (mut write, mut read) = socket.split();
            let mut interval = interval(Duration::from_secs(config.interval));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Collect system information
                        let system_data = monitor::collect_system_info(&mut system);
                        let network_data = monitor::collect_network_info(&mut networks);
                        let disk_data = monitor::collect_disk_info(&mut disks);
                        let data = monitor::ReportData {
                            uptime: System::uptime(),
                            system: system_data,
                            network: network_data,
                            disk: disk_data,
                        };
                        let msg = api::Message {
                            r#type: "report".to_string(),
                            data
                        };

                        // Send system information to WebSocket
                        if let Err(e) = write.send(Message::Text(serde_json::to_string(&msg).unwrap().into())).await {
                            warn!(error = %e, "Failed to report system data, attempting reconnect");
                            break; // Exit the current loop and reconnect.
                        }
                    }
                    Some(msg) = read.next() => {
                        match msg {
                            Ok(Message::Text(text)) => {
                                info!(message = %text, "Received WebSocket message");
                                match serde_json::from_str::<serde_json::Value>(&text) {
                                    Ok(json) => info!(json = ?json, "Parsed WebSocket message"),
                                    Err(e) => warn!(error = %e, "Failed to parse WebSocket message as JSON"),
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "WebSocket read error, attempting reconnect");
                                break; // Exit the current loop and reconnect.
                            }
                            _ => {}
                        }
                    }
                }
            }

            warn!("WebSocket connection lost, reconnecting...");
            retry_count += 1;
            let delay = config.connection.base_delay * 2u64.pow(retry_count.min(16) as u32 - 1);
            let delay = delay.min(config.connection.max_delay);
            sleep(Duration::from_secs(delay)).await;
        }
    };

    // Run WebSocket maintenance tasks and exit listening simultaneously
    tokio::select! {
        _ = shutdown_signal => {
            info!("Shutting down...");
        }
        _ = maintain_connection => {
            warn!("WebSocket maintenance task exited unexpectedly");
        }
    }
}
