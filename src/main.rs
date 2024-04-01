use anyhow::Result;
use clap::Parser;
use config::LurkConfig;
use log4rs::config::Deserializers;
use server::LurkServer;
use std::net::{IpAddr, SocketAddr};

mod auth;
mod client;
mod config;
mod error;
mod proto;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    log4rs::init_file(config::LOG4RS_CONFIG_FILE_PATH, Deserializers::default()).unwrap();
    // Parse config
    let config = LurkConfig::parse();
    // Create server
    let server_addr = SocketAddr::new(IpAddr::V4(config.ipv4()), config.port());
    let server = LurkServer::new(server_addr, config.auth_enabled());
    // Bind and serve clients "forever"
    server.run().await?;
    Ok(())
}
