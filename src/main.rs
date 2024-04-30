use anyhow::Result;
use clap::Parser;
use log4rs::config::Deserializers;
use lurk::{
    config::{self, LurkConfig},
    server::LurkServer,
};
use std::net::{IpAddr, SocketAddr};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    log4rs::init_file(config::LOG4RS_CONFIG_FILE_PATH, Deserializers::default()).unwrap();

    // Parse server config
    let config = LurkConfig::parse();

    // Create proxy server instance. It will handle incoming connection in async. fashion.
    let server_addr = SocketAddr::new(IpAddr::V4(config.bind_ipv4()), config.bind_port());
    let server = LurkServer::new(server_addr, config.tcp_conn_limit());

    // Bind and serve clients "forever"
    server.run().await?;

    Ok(())
}
