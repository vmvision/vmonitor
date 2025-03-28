use clap::Subcommand;
use std::env;
use tracing::error;

use crate::config;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show version information
    Version,

    /// List all configured endpoints
    List,

    /// Add a new endpoint
    Add {
        /// Name of the endpoint
        #[arg(short, long)]
        name: String,

        /// WebSocket URL
        #[arg(short, long)]
        server: String,

        /// Authentication secret
        #[arg(short, long)]
        secret: String,

        /// Whether to enable the endpoint immediately
        #[arg(short, long, default_value = "true")]
        enabled: bool,
    },

    /// Remove an endpoint
    Remove {
        /// Name of the endpoint to remove
        #[arg(short, long)]
        name: String,
    },

    /// Enable an endpoint
    Enable {
        /// Name of the endpoint to enable
        #[arg(short, long)]
        name: String,
    },

    /// Disable an endpoint
    Disable {
        /// Name of the endpoint to disable
        #[arg(short, long)]
        name: String,
    },
}

pub fn handle_command(command: Commands, config_path: &str) -> std::process::ExitCode {
    match command {
        Commands::List => {
            // Load configuration from config file
            let config = match config::AppConfig::from_file(config_path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!(error = %e, "Failed to load config");
                    return std::process::ExitCode::FAILURE;
                }
            };

            println!("Configured endpoints:");
            for endpoint in &config.endpoints {
                println!(
                    "  - {} ({})",
                    endpoint.name,
                    if endpoint.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            std::process::ExitCode::SUCCESS
        }
        Commands::Version => {
            println!("vmonitor {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Commands::Add { name, server, secret, enabled } => {
            let mut config = match config::AppConfig::from_file(config_path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!(error = %e, "Failed to load config");
                    return std::process::ExitCode::FAILURE;
                }
            };

            // Check if endpoint with same name already exists
            if config.endpoints.iter().any(|e| e.name == name) {
                error!("Endpoint with name '{}' already exists", name);
                return std::process::ExitCode::FAILURE;
            }

            // Add new endpoint
            config.endpoints.push(config::Endpoint {
                name,
                server,
                secret,
                enabled,
                connection: None,
            });

            // Save updated config
            if let Err(e) = config.save_to_file(config_path) {
                error!(error = %e, "Failed to save config");
                return std::process::ExitCode::FAILURE;
            }

            println!("Endpoint added successfully");
            std::process::ExitCode::SUCCESS
        }
        Commands::Remove { name } => {
            let mut config = match config::AppConfig::from_file(config_path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!(error = %e, "Failed to load config");
                    return std::process::ExitCode::FAILURE;
                }
            };

            // Find and remove endpoint
            if let Some(pos) = config.endpoints.iter().position(|e| e.name == name) {
                config.endpoints.remove(pos);
                
                // Save updated config
                if let Err(e) = config.save_to_file(config_path) {
                    error!(error = %e, "Failed to save config");
                    return std::process::ExitCode::FAILURE;
                }
                println!("Endpoint removed successfully");
                std::process::ExitCode::SUCCESS
            } else {
                error!("Endpoint with name '{}' not found", name);
                std::process::ExitCode::FAILURE
            }
        }
        Commands::Enable { name } => {
            let mut config = match config::AppConfig::from_file(config_path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!(error = %e, "Failed to load config");
                    return std::process::ExitCode::FAILURE;
                }
            };

            // Find and enable endpoint
            if let Some(endpoint) = config.endpoints.iter_mut().find(|e| e.name == name) {
                endpoint.enabled = true;
                
                // Save updated config
                if let Err(e) = config.save_to_file(config_path) {
                    error!(error = %e, "Failed to save config");
                    return std::process::ExitCode::FAILURE;
                }
                println!("Endpoint enabled successfully");
                std::process::ExitCode::SUCCESS
            } else {
                error!("Endpoint with name '{}' not found", name);
                std::process::ExitCode::FAILURE
            }
        }
        Commands::Disable { name } => {
            let mut config = match config::AppConfig::from_file(config_path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!(error = %e, "Failed to load config");
                    return std::process::ExitCode::FAILURE;
                }
            };

            // Find and disable endpoint
            if let Some(endpoint) = config.endpoints.iter_mut().find(|e| e.name == name) {
                endpoint.enabled = false;
                
                // Save updated config
                if let Err(e) = config.save_to_file(config_path) {
                    error!(error = %e, "Failed to save config");
                    return std::process::ExitCode::FAILURE;
                }
                println!("Endpoint disabled successfully");
                std::process::ExitCode::SUCCESS
            } else {
                error!("Endpoint with name '{}' not found", name);
                std::process::ExitCode::FAILURE
            }
        }
    }
} 