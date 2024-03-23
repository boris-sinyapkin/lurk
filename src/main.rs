use anyhow::Result;
use clap::Parser;
use config::LurkConfig;
use log4rs::config::Deserializers;
use server::LurkServer;

mod auth;
mod client;
mod config;
mod proto;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    log4rs::init_file(config::LOG4RS_CONFIG_FILE_PATH, Deserializers::default()).unwrap();
    // Parse config
    let config = LurkConfig::parse();
    // Create server
    let server = LurkServer::new(config.ipv4(), config.port(), config.auth_enabled());
    // Bind and serve clients "forever"
    server.run().await?;
    Ok(())
}
