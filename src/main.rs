use anyhow::Result;
use clap::Parser;
use log::error;
use log4rs::config::Deserializers;
use lurk::{
    api::LurkHttpEndpoint,
    config::{self, LurkConfig},
    server::LurkServer,
};
use std::net::{Ipv4Addr, SocketAddrV4};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    log4rs::init_file(config::LOG4RS_CONFIG_FILE_PATH, Deserializers::default()).unwrap();

    // Parse config
    let lurk_config = LurkConfig::parse();

    // Create proxy server instance. It will handle incoming connection in async. fashion.
    let server = LurkServer::new(lurk_config.server_tcp_bind_addr());

    if lurk_config.enable_http_endpoint() {
        let http_endpoint_addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8112);
        let http_endpoint = LurkHttpEndpoint::new(http_endpoint_addr);
        tokio::spawn(async move {
            if let Err(err) = http_endpoint.run().await {
                error!("Error occured while HTTP endpoint was running: {}", err);
            }
        });
    }

    // Bind and serve clients "forever"
    server.run().await?;

    Ok(())
}
