mod common;

use vmonitor::config::AppConfig;
use common::TestConfig;

#[test]
fn test_config_parsing() {
    let config_str = r#"
        interval = 60

        [connection]
        base_delay = 2
        max_delay = 120
        max_retries = 3

        [[endpoints]]
        name = "test-endpoint"
        websocket_url = "wss://test.example.com/ws"
        auth_secret = "test-secret"
        enabled = true

        [[endpoints]]
        name = "disabled-endpoint"
        websocket_url = "wss://disabled.example.com/ws"
        auth_secret = "disabled-secret"
        enabled = false
    "#;

    let test_config = TestConfig::new();
    std::fs::write(&test_config.config_path, config_str).unwrap();

    // Parse config
    let config = AppConfig::from_file(test_config.config_path.to_str().unwrap()).unwrap();

    // Verify config values
    assert_eq!(config.interval, 60);
    assert_eq!(config.connection.base_delay, 2);
    assert_eq!(config.connection.max_delay, 120);
    assert_eq!(config.connection.max_retries, 3);
    assert_eq!(config.endpoints.len(), 2);

    // Verify first endpoint
    let endpoint = &config.endpoints[0];
    assert_eq!(endpoint.name, "test-endpoint");
    assert_eq!(endpoint.websocket_url, "wss://test.example.com/ws");
    assert_eq!(endpoint.auth_secret, "test-secret");
    assert!(endpoint.enabled);

    // Verify second endpoint
    let endpoint = &config.endpoints[1];
    assert_eq!(endpoint.name, "disabled-endpoint");
    assert!(!endpoint.enabled);
} 