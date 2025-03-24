use std::path::PathBuf;
use std::mem;
use tempfile::TempDir;

pub struct TestConfig {
    pub temp_dir: TempDir,
    pub config_path: PathBuf,
}

impl TestConfig {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        Self {
            temp_dir,
            config_path,
        }
    }
}

impl Drop for TestConfig {
    fn drop(&mut self) {
        // Clean up temp directory by replacing it with a dummy value
        let _ = mem::replace(&mut self.temp_dir, tempfile::tempdir().unwrap()).close();
    }
} 