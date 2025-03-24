use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_cli_version() {
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
        .arg(config_path)
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test"));
    assert!(stdout.contains("enabled"));
}

#[test]
fn test_cli_invalid_config() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("invalid_config.toml");
    
    // Create invalid config file
    std::fs::write(&config_path, "invalid toml content").unwrap();

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("--config")
        .arg(config_path)
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
} 