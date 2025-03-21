mod api;
mod config;
mod monitor;

use futures_util::{SinkExt, StreamExt};
use std::env;
use sysinfo::{Disks, Networks, System};
use tokio::signal;
use tokio::time::{interval, sleep, Duration};
use tokio_tungstenite::tungstenite::{Bytes, Message};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
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
                        monitor::send_metrics(&mut write, &mut system, &mut networks, &mut disks).await;
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
                            Ok(Message::Binary(binary)) => {
                                info!(binary = ?binary, "Received binary message");
                                match rmp_serde::from_slice::<api::Message<serde_json::Value>>(&binary) {
                                    Ok(api_msg) => {
                                        if api_msg.r#type == "get_info" {
                                            let vm_info = monitor::collect_vm_info(&mut system, &mut disks);
                                            info!(vm_info = ?vm_info, "Sending VM info response");
                                            let response = api::Message {
                                                r#type: "vm_info".to_string(),
                                                data: vm_info,
                                            };
                                            if let Ok(msgpack) = rmp_serde::to_vec_named(&response) {
                                                if let Err(e) = write.send(Message::Binary(Bytes::from(msgpack))).await {
                                                    warn!(error = %e, "Failed to send VM info response");
                                                }
                                                info!("Sent VM info response");
                                            }
                                        }
                                        info!(message = ?api_msg, "Parsed MessagePack message");
                                    }
                                    Err(e) => warn!(error = %e, "Failed to parse as api::Message"),
                                }
                            }
                            Ok(Message::Ping(ping)) => {
                                if let Err(e) = write.send(Message::Pong(ping)).await {
                                    error!(error = %e, "Failed to send pong response");
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
