use vmonitor::app::App;
use vmonitor::config::{AppConfig, EndpointConfig, ConnectionConfig};
use tokio::time::Duration;

#[tokio::test]
async fn test_app_startup_shutdown() {
    // Create test config
    let config = AppConfig {
        metrics_interval: 1,
        ip_report_interval: 1,
        connection: ConnectionConfig {
            base_delay: 1,
            max_delay: 5,
            max_retries: 1,
        },
        endpoints: vec![
            EndpointConfig {
                name: "test".to_string(),
                server: "wss://test.example.com/ws".to_string(),
                secret: "test-secret".to_string(),
                enabled: true,
                metrics_interval: None,
                ip_report_interval: None,
                connection: None,
            }
        ],
    };

    // Create app instance
    let app = App::new(config);

    // Run app with timeout
    let app_handle = tokio::spawn(async move {
        app.run().await
    });

    // Wait for a short period and then send shutdown signal
    tokio::time::sleep(Duration::from_secs(1)).await;
    tokio::signal::ctrl_c().await.unwrap();

    // Verify app shuts down cleanly
    tokio::time::timeout(Duration::from_secs(2), app_handle)
        .await
        .expect("App failed to shutdown")
        .expect("App panicked");
}

#[tokio::test]
async fn test_app_with_disabled_endpoints() {
    // Create test config with disabled endpoint
    let config = AppConfig {
        metrics_interval: 1,
        ip_report_interval: 1,
        connection: ConnectionConfig {
            base_delay: 1,
            max_delay: 5,
            max_retries: 1,
        },
        endpoints: vec![
            EndpointConfig {
                name: "disabled".to_string(),
                server: "wss://test.example.com/ws".to_string(),
                secret: "test-secret".to_string(),
                enabled: false,
                metrics_interval: None,
                ip_report_interval: None,
                connection: None,
            }
        ],
    };

    // Create app instance
    let app = App::new(config);

    // Run app with timeout
    let app_handle = tokio::spawn(async move {
        app.run().await
    });

    // Wait for a short period and then send shutdown signal
    tokio::time::sleep(Duration::from_secs(1)).await;
    tokio::signal::ctrl_c().await.unwrap();

    // Verify app shuts down cleanly
    tokio::time::timeout(Duration::from_secs(2), app_handle)
        .await
        .expect("App failed to shutdown")
        .expect("App panicked");
} 