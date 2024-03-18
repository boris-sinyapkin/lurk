use anyhow::Result;
use clap::Parser;
use log4rs::config::Deserializers;
use server::LurkServer;

mod config;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    log4rs::init_file(config::LOG4RS_CONFIG_FILE_PATH, Deserializers::default()).unwrap();
    // Parse config
    let config = config::LurkConfig::parse();
    // Create server
    let server = LurkServer::new(config.ipv4(), config.port()).await?;
    // Run "forever"
    server.run().await;
    Ok(())
}
