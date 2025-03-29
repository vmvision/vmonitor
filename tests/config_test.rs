mod common;

use std::fs;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::sleep;
use vmonitor::app::App;
use vmonitor::config::{AppConfig, Endpoint, ConnectionConfig};
use common::TestConfig;

fn create_default_config() -> AppConfig {
    AppConfig {
        endpoints: vec![],
        connection: ConnectionConfig {
            base_delay: 1,
            max_delay: 60,
            max_retries: -1,
        },
    }
}

#[test]
fn test_endpoint_with_defaults() {
    let default_config = create_default_config();
    let endpoint = Endpoint {
        name: "test".to_string(),
        server: "ws://test.com".to_string(),
        secret: "test-secret".to_string(),
        enabled: true,
        connection: None,
    };

    assert_eq!(
        endpoint.connection.clone().unwrap_or_else(|| default_config.connection.clone()),
        default_config.connection
    );
}

#[test]
fn test_endpoint_with_overrides() {
    let default_config = create_default_config();
    let custom_connection = ConnectionConfig {
        base_delay: 2,
        max_delay: 30,
        max_retries: 3,
    };

    let endpoint = Endpoint {
        name: "test".to_string(),
        server: "ws://test.com".to_string(),
        secret: "test-secret".to_string(),
        enabled: true,
        connection: Some(custom_connection.clone()),
    };

    assert_eq!(endpoint.connection.clone().unwrap_or_else(|| default_config.connection.clone()), custom_connection);
}

#[test]
fn test_config_serialization() {
    let config = AppConfig {
        endpoints: vec![
            Endpoint {
                name: "test1".to_string(),
                server: "ws://test1.com".to_string(),
                secret: "secret1".to_string(),
                enabled: true,
                connection: None,
            },
            Endpoint {
                name: "test2".to_string(),
                server: "ws://test2.com".to_string(),
                secret: "secret2".to_string(),
                enabled: true,
                connection: Some(ConnectionConfig {
                    base_delay: 2,
                    max_delay: 30,
                    max_retries: 3,
                }),
            },
        ],
        connection: ConnectionConfig {
            base_delay: 1,
            max_delay: 60,
            max_retries: -1,
        },
    };

    let serialized = toml::to_string_pretty(&config).unwrap();
    let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
    assert_eq!(config, deserialized);
}

#[test]
fn test_config_parsing() {
    let config_str = r#"
        [connection]
        base_delay = 2
        max_delay = 120
        max_retries = 3

        [[endpoints]]
        name = "test-endpoint"
        server = "wss://test.example.com/ws"
        secret = "test-secret"
        enabled = true

        [[endpoints]]
        name = "disabled-endpoint"
        server = "wss://disabled.example.com/ws"
        secret = "disabled-secret"
        enabled = false
    "#;

    let test_config = TestConfig::new();
    std::fs::write(&test_config.config_path, config_str).unwrap();

    // Parse config
    let config = AppConfig::from_file(test_config.config_path.to_str().unwrap()).unwrap();

    // Verify config values
    assert_eq!(config.connection.base_delay, 2);
    assert_eq!(config.connection.max_delay, 120);
    assert_eq!(config.connection.max_retries, 3);
    assert_eq!(config.endpoints.len(), 2);

    // Verify first endpoint
    let endpoint = &config.endpoints[0];
    assert_eq!(endpoint.name, "test-endpoint");
    assert_eq!(endpoint.server, "wss://test.example.com/ws");
    assert_eq!(endpoint.secret, "test-secret");
    assert!(endpoint.enabled);

    // Verify second endpoint
    let endpoint = &config.endpoints[1];
    assert_eq!(endpoint.name, "disabled-endpoint");
    assert!(!endpoint.enabled);
}

#[tokio::test]
async fn test_dynamic_endpoint_management() {
    // Create a temporary directory for our test config
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");

    // Create initial config with one endpoint
    let initial_config = AppConfig {
        endpoints: vec![
            Endpoint {
                name: "test1".to_string(),
                server: "wss://test1.example.com/ws".to_string(),
                secret: "secret1".to_string(),
                enabled: true,
                connection: None,
            }
        ],
        connection: ConnectionConfig {
            base_delay: 1,
            max_delay: 5,
            max_retries: 1,
        },
    };

    // Save initial config
    initial_config.save_to_file(config_path.to_str().unwrap()).unwrap();

    // Create app instance
    let app = App::new(initial_config);

    // Spawn app in background
    let app_handle = tokio::spawn(async move {
        app.run().await;
    });

    // Wait for app to start
    sleep(Duration::from_secs(1)).await;

    // Test 1: Add new endpoint
    let mut config = AppConfig::from_file(config_path.to_str().unwrap()).unwrap();
    config.endpoints.push(Endpoint {
        name: "test2".to_string(),
        server: "wss://test2.example.com/ws".to_string(),
        secret: "secret2".to_string(),
        enabled: true,
        connection: None,
    });
    config.save_to_file(config_path.to_str().unwrap()).unwrap();

    // Wait for config to be reloaded
    sleep(Duration::from_secs(2)).await;

    // Test 2: Disable endpoint
    let mut config = AppConfig::from_file(config_path.to_str().unwrap()).unwrap();
    if let Some(endpoint) = config.endpoints.iter_mut().find(|e| e.name == "test1") {
        endpoint.enabled = false;
    }
    config.save_to_file(config_path.to_str().unwrap()).unwrap();

    // Wait for config to be reloaded
    sleep(Duration::from_secs(2)).await;

    // Test 3: Enable endpoint
    let mut config = AppConfig::from_file(config_path.to_str().unwrap()).unwrap();
    if let Some(endpoint) = config.endpoints.iter_mut().find(|e| e.name == "test1") {
        endpoint.enabled = true;
    }
    config.save_to_file(config_path.to_str().unwrap()).unwrap();

    // Wait for config to be reloaded
    sleep(Duration::from_secs(2)).await;

    // Test 4: Remove endpoint
    let mut config = AppConfig::from_file(config_path.to_str().unwrap()).unwrap();
    config.endpoints.retain(|e| e.name != "test2");
    config.save_to_file(config_path.to_str().unwrap()).unwrap();

    // Wait for config to be reloaded
    sleep(Duration::from_secs(2)).await;

    // Clean up
    app_handle.abort();
    let _ = app_handle.await;
}

#[tokio::test]
async fn test_config_file_monitoring() {
    // Create a temporary directory for our test config
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");

    // Create initial config
    let initial_config = AppConfig {
        endpoints: vec![
            Endpoint {
                name: "test".to_string(),
                server: "wss://test.example.com/ws".to_string(),
                secret: "secret".to_string(),
                enabled: true,
                connection: None,
            }
        ],
        connection: ConnectionConfig {
            base_delay: 1,
            max_delay: 5,
            max_retries: 1,
        },
    };

    // Save initial config
    initial_config.save_to_file(config_path.to_str().unwrap()).unwrap();

    // Create app instance
    let app = App::new(initial_config);

    // Spawn app in background
    let app_handle = tokio::spawn(async move {
        app.run().await;
    });

    // Wait for app to start
    sleep(Duration::from_secs(1)).await;

    // Test: Corrupt config file
    fs::write(&config_path, "invalid toml content").unwrap();
    
    // Wait for config monitoring to detect the change
    sleep(Duration::from_secs(2)).await;

    // Restore valid config
    let valid_config = AppConfig {
        endpoints: vec![
            Endpoint {
                name: "test".to_string(),
                server: "wss://test.example.com/ws".to_string(),
                secret: "secret".to_string(),
                enabled: true,
                connection: None,
            }
        ],
        connection: ConnectionConfig {
            base_delay: 1,
            max_delay: 5,
            max_retries: 1,
        },
    };
    valid_config.save_to_file(config_path.to_str().unwrap()).unwrap();

    // Wait for config to be reloaded
    sleep(Duration::from_secs(2)).await;

    // Clean up
    app_handle.abort();
    let _ = app_handle.await;
}

#[tokio::test]
async fn test_concurrent_config_changes() {
    // Create a temporary directory for our test config
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");

    // Create initial config
    let initial_config = AppConfig {
        endpoints: vec![
            Endpoint {
                name: "test".to_string(),
                server: "wss://test.example.com/ws".to_string(),
                secret: "secret".to_string(),
                enabled: true,
                connection: None,
            }
        ],
        connection: ConnectionConfig {
            base_delay: 1,
            max_delay: 5,
            max_retries: 1,
        },
    };

    // Save initial config
    initial_config.save_to_file(config_path.to_str().unwrap()).unwrap();

    // Create app instance
    let app = App::new(initial_config);

    // Spawn app in background
    let app_handle = tokio::spawn(async move {
        app.run().await;
    });

    // Wait for app to start
    sleep(Duration::from_secs(1)).await;

    // Test: Make rapid config changes
    for i in 0..5 {
        let mut config = AppConfig::from_file(config_path.to_str().unwrap()).unwrap();
        config.endpoints[0].enabled = i % 2 == 0;
        config.save_to_file(config_path.to_str().unwrap()).unwrap();
        sleep(Duration::from_millis(100)).await;
    }

    // Wait for final config to be reloaded
    sleep(Duration::from_secs(2)).await;

    // Clean up
    app_handle.abort();
    let _ = app_handle.await;
} 