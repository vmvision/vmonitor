use std::process::Command;
use tempfile::tempdir;
use tracing_subscriber::{fmt, EnvFilter};

fn setup() {
    // Set up tracing subscriber to output to stderr
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
}

#[test]
fn test_cli_version() {
    setup();
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("vmonitor"));
}

#[test]
fn test_cli_list_endpoints() {
    setup();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    
    // Create test config file
    std::fs::write(
        &config_path,
        r#"
        interval = 60
        [[endpoints]]
        name = "test"
        websocket_url = "wss://test.example.com/ws"
        auth_secret = "test-secret"
        enabled = true
        "#,
    )
    .unwrap();

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test"));
    assert!(stdout.contains("enabled"));
}

#[test]
fn test_cli_add_endpoint() {
    setup();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    
    // Create initial config file
    std::fs::write(
        &config_path,
        r#"
        interval = 60
        "#,
    )
    .unwrap();

    // Test adding a new endpoint
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("add")
        .arg("--name")
        .arg("new-endpoint")
        .arg("--url")
        .arg("ws://example.com/ws")
        .arg("--secret")
        .arg("test-secret")
        .arg("--enabled")
        .arg("true")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Endpoint added successfully"));

    // Verify the endpoint was added by listing endpoints
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("new-endpoint"));
    assert!(stdout.contains("enabled"));
}

#[test]
fn test_cli_remove_endpoint() {
    setup();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    
    // Create test config file with an endpoint
    std::fs::write(
        &config_path,
        r#"
        interval = 60
        [[endpoints]]
        name = "test"
        websocket_url = "wss://test.example.com/ws"
        auth_secret = "test-secret"
        enabled = true
        "#,
    )
    .unwrap();

    // Test removing the endpoint
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("remove")
        .arg("--name")
        .arg("test")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Endpoint removed successfully"));

    // Verify the endpoint was removed by listing endpoints
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("test"));
}

#[test]
fn test_cli_enable_disable_endpoint() {
    setup();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    
    // Create test config file with a disabled endpoint
    std::fs::write(
        &config_path,
        r#"
        interval = 60
        [[endpoints]]
        name = "test"
        websocket_url = "wss://test.example.com/ws"
        auth_secret = "test-secret"
        enabled = false
        "#,
    )
    .unwrap();

    // Test enabling the endpoint
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("enable")
        .arg("--name")
        .arg("test")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Endpoint enabled successfully"));

    // Verify the endpoint is enabled
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test"));
    assert!(stdout.contains("enabled"));

    // Test disabling the endpoint
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("disable")
        .arg("--name")
        .arg("test")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Endpoint disabled successfully"));

    // Verify the endpoint is disabled
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test"));
    assert!(stdout.contains("disabled"));
}

#[test]
fn test_cli_invalid_config() {
    setup();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("invalid_config.toml");
    
    // Create invalid config file
    std::fs::write(&config_path, "invalid toml content").unwrap();

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
}

#[test]
fn test_cli_duplicate_endpoint() {
    setup();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    
    // Create test config file with an endpoint
    std::fs::write(
        &config_path,
        r#"
        interval = 60
        [[endpoints]]
        name = "test"
        websocket_url = "wss://test.example.com/ws"
        auth_secret = "test-secret"
        enabled = true
        "#,
    )
    .unwrap();

    // Try to add an endpoint with the same name
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("add")
        .arg("--name")
        .arg("test")
        .arg("--url")
        .arg("ws://example.com/ws")
        .arg("--secret")
        .arg("test-secret")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
}

#[test]
fn test_cli_nonexistent_endpoint() {
    setup();
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("test_config.toml");
    
    // Create test config file without endpoints
    std::fs::write(
        &config_path,
        r#"
        interval = 60
        "#,
    )
    .unwrap();

    // Try to remove a non-existent endpoint
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("remove")
        .arg("--name")
        .arg("nonexistent")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));

    // Try to enable a non-existent endpoint
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("enable")
        .arg("--name")
        .arg("nonexistent")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));

    // Try to disable a non-existent endpoint
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(&config_path)
        .arg("disable")
        .arg("--name")
        .arg("nonexistent")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));
} 