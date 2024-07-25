use anyhow::Result;
use clap::Parser;
use log::error;
use log4rs::config::Deserializers;
use lurk::{
    api::LurkHttpEndpoint,
    config::{self, LurkConfig},
    server::LurkServer,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    log4rs::init_file(config::LOG4RS_CONFIG_FILE_PATH, Deserializers::default()).unwrap();

    // Parse config
    let lurk_config = LurkConfig::parse();

    // Create proxy server instance. It will handle incoming connection in async. fashion.
    let server = LurkServer::new(lurk_config.server_tcp_bind_addr());

    // Spin up HTTP endpoint if enabled
    if let Some(http_endpoint_bind_addr) = lurk_config.http_endpoint_bind_addr() {
        let http_endpoint = LurkHttpEndpoint::new(http_endpoint_bind_addr);
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
