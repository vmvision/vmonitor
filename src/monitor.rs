use crate::api;
use crate::config::Endpoint;
use crate::features::metrics::Metrics;
use futures::{sink::SinkExt, stream::StreamExt};
use futures_util::stream::SplitStream;
use tokio::{
    net::TcpStream,
    sync::{mpsc, watch},
    time::{interval, sleep, Duration},
};
use tokio_tungstenite::{
    tungstenite::{Bytes, Message},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, warn};

#[derive(Clone)]
struct Config {
    metrics_interval: Duration,
}
impl Config {
    fn new() -> Self {
        Self {
            metrics_interval: Duration::from_secs(10),
        }
    }
    fn validate(&self) -> Result<(), String> {
        if self.metrics_interval < Duration::from_secs(1) {
            return Err("Metrics interval must be at least 1 second".to_string());
        }
        Ok(())
    }
}
pub struct Monitor {
    pub endpoint: Endpoint,
    config_tx: watch::Sender<Config>,
    config_rx: watch::Receiver<Config>,
}

enum WriteMessage {
    Data(Vec<u8>),
    Pong(Bytes),
    Close,
}

impl Monitor {
    pub fn new(endpoint: Endpoint) -> Self {
        let (config_tx, config_rx) = watch::channel(Config::new());
        Self {
            endpoint,
            config_tx,
            config_rx,
        }
    }

    pub async fn run(&self) {
        let mut retry_count = 0;

        loop {
            let endpoint = self.endpoint.clone();
            let strategy = endpoint.connection.unwrap();

            let socket = match api::connect_websocket(
                endpoint.server.as_str(),
                endpoint.secret.as_str(),
                &strategy,
            )
            .await
            {
                Some(socket) => socket,
                None => {
                    return;
                }
            };
            let (mut write, mut read) = socket.split();
            let (tx, mut rx) = mpsc::channel::<WriteMessage>(100);

            let write_task = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    match msg {
                        WriteMessage::Data(data) => {
                            if let Err(e) = write.send(Message::Binary(Bytes::from(data))).await {
                                eprintln!("Write error: {}", e);
                                break;
                            }
                        }
                        WriteMessage::Pong(data) => {
                            if let Err(e) = write.send(Message::Pong(Bytes::from(data))).await {
                                eprintln!("Write error: {}", e);
                                break;
                            }
                        }
                        WriteMessage::Close => {
                            if let Err(e) = write.send(Message::Close(None)).await {
                                eprintln!("Write error: {}", e);
                            }
                            break;
                        }
                    }
                }
            });
            let send_metrics_tx = tx.clone();
            let metrics_config_rx = self.config_rx.clone();
            let send_metrics_task = tokio::spawn(async move {
                Monitor::send_metrics(send_metrics_tx, metrics_config_rx).await;
            });
            let command_handle_tx = tx.clone();
            let config_tx = self.config_tx.clone();
            let command_handle_task = tokio::spawn(async move {
                Monitor::handle_command(&endpoint, &mut read, command_handle_tx, config_tx).await
            });

            let _ = tokio::try_join!(write_task, send_metrics_task, command_handle_task);

            retry_count += 1;
            if strategy.max_retries >= 0 && retry_count > strategy.max_retries {
                let delay = strategy.base_delay * 2u64.pow(retry_count.min(16) as u32 - 1);
                let delay = delay.min(strategy.max_delay);

                debug!(
                    "Operation failed (attempt {}), retrying in {} seconds",
                    retry_count, delay
                );
                sleep(Duration::from_secs(delay)).await;
            }
        }
    }

    async fn send_metrics(tx: mpsc::Sender<WriteMessage>, mut config_rx: watch::Receiver<Config>) {
        let mut metrics_interval = interval(config_rx.borrow().metrics_interval);
        let mut metrics = Metrics::new();

        loop {
            tokio::select! {
                result = config_rx.changed() => {
                    if result.is_ok() {
                        metrics_interval = interval(config_rx.borrow().metrics_interval);
                        debug!("Metrics interval updated to {:?}", config_rx.borrow().metrics_interval);
                    }
                }
                _ = metrics_interval.tick() => {
                    let data = metrics.collet_metrics().await;
                    let msg = api::Message {
                        r#type: "metrics".to_string(),
                        data,
                    };
                    match rmp_serde::to_vec_named(&msg) {
                        Ok(binary_data) => {
                            if let Err(e) = tx.send(WriteMessage::Data(binary_data)).await {
                                warn!(error = %e, "Failed to report system data");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to serialize system data");
                        }
                    }
                }
            }
        }
    }

    async fn handle_command(
        endpoint: &Endpoint,
        read: &mut SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        tx: mpsc::Sender<WriteMessage>,
        config_tx: watch::Sender<Config>,
    ) {
        let mut metrics = Metrics::new();

        loop {
            let msg = read.next().await;
            let Some(msg) = msg else {
                break;
            };
            let command = match msg {
                Ok(Message::Text(text)) => {
                    debug!(endpoint = %endpoint.name, message = %text, "Received WebSocket message");
                    match serde_json::from_str::<api::Message<serde_json::Value>>(&text) {
                        Ok(value) => {
                            debug!(endpoint = %endpoint.name, json = ?value, "Parsed WebSocket message");
                            Some(value)
                        }
                        Err(e) => {
                            warn!(endpoint = %endpoint.name, error = %e, "Failed to parse WebSocket message as JSON");
                            None
                        }
                    }
                }
                Ok(Message::Binary(binary)) => {
                    debug!(endpoint = %endpoint.name, binary = ?binary, "Received binary message");
                    match rmp_serde::from_slice::<api::Message<serde_json::Value>>(&binary) {
                        Ok(api_msg) => Some(api_msg),
                        Err(e) => {
                            warn!(endpoint = %endpoint.name, error = %e, "Failed to parse as api::Message");
                            None
                        }
                    }
                }
                Ok(Message::Ping(ping)) => {
                    if let Err(e) = tx.send(WriteMessage::Pong(ping)).await {
                        error!(endpoint = %endpoint.name, error = %e, "Failed to send pong response");
                    }
                    None
                }
                _ => None,
            };

            match command {
                Some(value) => match value.r#type.as_str() {
                    "get_info" => {
                        let vm_info = metrics.collect_vm_info();
                        info!(endpoint = %endpoint.name, vm_info = ?vm_info, "Sending VM info response");
                        let response = api::Message {
                            r#type: "vm_info".to_string(),
                            data: vm_info,
                        };
                        if let Ok(msgpack) = rmp_serde::to_vec_named(&response) {
                            if let Err(e) = tx.send(WriteMessage::Data(msgpack)).await {
                                warn!(endpoint = %endpoint.name, error = %e, "Failed to send VM info response");
                            }
                            info!(endpoint = %endpoint.name, "Sent VM info response");
                        }
                    }
                    "update_config" => {
                        if let Ok(probe_config) =
                            serde_json::from_value::<api::ProbeConfig>(value.data)
                        {
                            info!(endpoint = %endpoint.name, config = ?probe_config, "Received server configuration");
                            let new_config = Config {
                                metrics_interval: Duration::from_secs(
                                    probe_config.metrics_interval,
                                ),
                            };
                            if let Err(e) = new_config.validate() {
                                warn!(error = %e, "Invalid configuration received");
                                continue;
                            }
                            if let Err(e) = config_tx.send(new_config) {
                                warn!(error = %e, "Failed to update configuration");
                            } else {
                                info!("Configuration updated successfully");
                            }
                        }
                    }
                    _ => {
                        info!(endpoint = %endpoint.name, message = ?value, "Received unknown message type")
                    }
                },
                None => {}
            }
        }
    }
}
