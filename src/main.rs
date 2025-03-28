mod api;
mod app;
mod cli;
mod config;
mod monitor;
mod metrics;

use clap::Parser;
use std::env;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(
    author = "AprilNEA <github@sku.moe>",
    version,
    about = "A simple and lightweight system monitor",
    long_about = "vmonitor is a system monitoring tool that collects system metrics and sends them to configured WebSocket endpoints.",
    after_help = "For more information, visit: https://github.com/vmvision/vmonitor"
)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Override the config file path with environment variable
    #[arg(short, long, default_value = "VMONITOR_CONFIG_PATH")]
    env_var: String,

    /// Set the logging level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Option<cli::Commands>,
}

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Parse command line arguments
    let args = Args::parse();

    // Initialize tracing subscriber with specified log level
    tracing_subscriber::fmt()
        .with_env_filter(&args.log_level)
        .init();

    // Get config path from environment variable or command line argument
    let config_path = env::var(&args.env_var).unwrap_or(args.config);

    // Handle subcommands first
    if let Some(command) = args.command {
        let exit_code = cli::handle_command(command, &config_path);
        std::process::exit(if exit_code == std::process::ExitCode::SUCCESS { 0 } else { 1 });
    }

    info!(config_path = %config_path, "Starting application");

    // Load configuration from config file
    let config = match config::AppConfig::from_file(&config_path) {
        Ok(mut cfg) => {
            for endpoint in cfg.endpoints.iter_mut() {
                if endpoint.connection.is_none() {
                    endpoint.connection = Some(cfg.connection);
                }
            }
            cfg
        },
        Err(e) => {
            error!(error = %e, "Failed to load config");
            std::process::exit(1);
        }
    };
    info!("Configuration loaded");

    // Create and run the application
    let app = app::App::new(config);
    app.run().await;
}
