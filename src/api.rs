use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::time::Duration;
use tokio_tungstenite::{
    connect_async,
    tungstenite::http::{uri, Uri},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{error, info, warn};

#[derive(Serialize, Deserialize, Debug)]
pub struct Message<T> {
    pub r#type: String,
    pub data: T,
}

pub struct ConnectionConfig {
    pub base_delay: u64,
    pub max_delay: u64,
    pub max_retries: i32,
}

pub async fn connect_websocket(
    ws_url: &str,
    auth_secret: &str,
    config: &ConnectionConfig,
) -> Option<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let mut retry_count = 0;
    loop {
        let mut uri_parts = Uri::from_str(ws_url).expect("Invalid URL").into_parts();
        uri_parts.path_and_query = Some(
            uri::PathAndQuery::from_str(&format!(
                "{}?auth_secret={}",
                uri_parts.path_and_query.unwrap(),
                auth_secret
            ))
            .unwrap(),
        );
        let uri = Uri::from_parts(uri_parts).expect("Invalid URL");

        match connect_async(uri).await {
            Ok((socket, _)) => {
                info!("WebSocket connection established");
                return Some(socket);
            }
            Err(e) => {
                error!(error = %e, url = %ws_url, "WebSocket connection failed");
                if let tokio_tungstenite::tungstenite::Error::Http(response) = &e {
                    if response.status() == 401 {
                        error!("Authentication failed - invalid or missing auth token");
                        return None;
                    }
                }
            }
        }

        // Check max retries
        if config.max_retries >= 0 && retry_count >= config.max_retries {
            error!(
                "Failed to connect to WebSocket after {} attempts",
                retry_count
            );
            return None;
        }

        retry_count += 1;
        // Calculate delay with exponential backoff, capped at max_delay
        let delay = config.base_delay * 2u64.pow(retry_count.min(16) as u32 - 1);
        let delay = delay.min(config.max_delay);

        warn!(
            retry = retry_count,
            next_attempt_in = delay,
            "WebSocket connection failed, retrying..."
        );

        tokio::time::sleep(Duration::from_secs(delay)).await;
    }
}
