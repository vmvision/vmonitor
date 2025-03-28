use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::time::Duration;
use tokio_tungstenite::{
    connect_async,
    tungstenite::http::{uri, Uri},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, warn};

use crate::config::ConnectionConfig;

#[derive(Serialize, Deserialize, Debug)]
pub struct Message<T> {
    pub r#type: String,
    pub data: T,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProbeConfig {
    pub metrics_interval: u64,
}

fn build_uri(server: &str, secret: &str) -> Uri {
    let mut uri_parts = Uri::from_str(server).expect("Invalid URL").into_parts();
    let path_and_query = uri_parts
        .path_and_query
        .as_ref()
        .map(|pq| {
            if pq.path() == "/" {
                "/wss/probe".to_string()
            } else {
                pq.to_string()
            }
        })
        .unwrap_or_else(|| "/wss/probe".to_string());

    uri_parts.path_and_query = Some(
        uri::PathAndQuery::from_str(&format!("{}?secret={}", path_and_query, secret)).unwrap(),
    );

    Uri::from_parts(uri_parts).expect("Invalid URL")
}

// Attempts to establish a WebSocket connection to the specified server with authentication.
// Returns Some(WebSocketStream) if successful, None if authentication fails or max retries exceeded.
//
// # Arguments
// * `server` - The WebSocket server URL (ws:// or wss://)
// * `secret` - Authentication secret/token
// * `config` - Connection retry configuration
//
// The function will automatically append the WebSocket path (/wss/master) and auth token
// if not already present in the URL. It implements exponential backoff for retries,
// starting at base_delay and doubling up to max_delay seconds between attempts.
pub async fn connect_websocket(
    server: &str,
    secret: &str,
    config: &ConnectionConfig,
) -> Option<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let base_delay = config.base_delay;
    let max_delay = config.max_delay;
    let max_retries = config.max_retries;

    let mut retry_count = 0;

    let uri = build_uri(server, secret);

    debug!(url = %uri, "Connecting to WebSocket...");

    loop {
        match connect_async(uri.clone()).await {
            Ok((socket, _)) => {
                debug!(url = %uri, "WebSocket connection established");
                return Some(socket);
            }
            Err(e) => {
                error!(error = %e, url = %server, "WebSocket connection failed");
                if let tokio_tungstenite::tungstenite::Error::Http(response) = &e {
                    if response.status() == 401 {
                        error!(url = %server,"Authentication failed - invalid or missing auth token");
                        return None;
                    }
                }
            }
        }

        // Check max retries
        if max_retries >= 0 && retry_count >= max_retries {
            error!(
                "Failed to connect to WebSocket after {} attempts",
                retry_count
            );
            return None;
        }

        retry_count += 1;
        // Calculate delay with exponential backoff, capped at max_delay
        let delay = base_delay * 2u64.pow(retry_count.min(16) as u32 - 1);
        let delay = delay.min(max_delay);

        warn!(
            retry = retry_count,
            next_attempt_in = delay,
            "WebSocket connection failed, retrying..."
        );

        tokio::time::sleep(Duration::from_secs(delay)).await;
    }
}
