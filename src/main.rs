mod api;
mod config;
mod monitor;

use futures_util::{SinkExt, StreamExt};
use std::env;
use sysinfo::{Networks, System};
use tokio::signal;
use tokio::time::{interval, Duration};
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
            error!("Failed to connect to WebSocket");
            return;
        }
    };
    info!("WebSocket connection established");

    let (mut write, mut read) = socket.split();

    let mut system = System::new();
    let mut networks = Networks::new();

    let mut system_interval = interval(Duration::from_secs(config.interval.system));
    let mut network_interval = interval(Duration::from_secs(config.interval.network));

    let collect_task = async {
        loop {
            tokio::select! {
                _ = system_interval.tick() => {
                    // Collect system information
                    let system_data = monitor::collect_system_info(&mut system);
                    info!(data = ?system_data, "Collected system information");

                    let msg = api::ReportMessage {
                        r#type: "system".to_string(),
                        data: serde_json::to_string(&system_data).unwrap(),
                    };

                    if let Err(e) = write.send(Message::Text(serde_json::to_string(&msg).unwrap().into())).await {
                        warn!(error = %e, "Failed to report system data");
                    }
                }
                _ = network_interval.tick() => {
                    // Collect network information
                    let network_data = monitor::collect_network_info(&mut networks);
                    info!(data = ?network_data, "Collected network information");

                    let msg = api::ReportMessage {
                        r#type: "network".to_string(),
                        data: serde_json::to_string(&network_data).unwrap(),
                    };

                    if let Err(e) = write.send(Message::Text(serde_json::to_string(&msg).unwrap().into())).await {
                        warn!(error = %e, "Failed to report network data");
                    }
                }
                Some(msg) = read.next() => {
                    match msg {
                        Ok(msg) => {
                            if let Message::Text(text) = msg {
                                info!(message = %text, "Received WebSocket message");
                                match serde_json::from_str::<serde_json::Value>(&text) {
                                    Ok(json) => {
                                        info!(json = ?json, "Parsed WebSocket message");
                                    }
                                    Err(e) => {
                                        warn!(error = %e, "Failed to parse WebSocket message as JSON");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "WebSocket read error");
                            break; // Exit the loop when a connection error occurs.
                        }
                    }
                }
            }
        }
    };

    // Run data collection and exit monitoring in parallel.
    tokio::select! {
        _ = shutdown_signal => {
            info!("Shutting down...");
        }
        _ = collect_task => {
            warn!("Collect task exited unexpectedly");
        }
    }
}
