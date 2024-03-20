use anyhow::Result;
use log::{debug, info};
use std::net::SocketAddr;
use tokio::net::TcpStream;

use crate::proto::socks5::AuthMethodRequest;

pub struct LurkConnection {
    stream: TcpStream
}

impl LurkConnection {
    pub fn new(stream: TcpStream) -> LurkConnection {
        LurkConnection { stream }
    }

    pub async fn iterate(&mut self) -> Result<()> {
        // Before relaying the data, client should 
        // pass the authentification phase.
        self.auth_client().await?;
        

        Ok(())
    }

    async fn auth_client(&mut self) -> Result<()> {
        debug!("Authentication phase has started");
        // Handle auth method request. It contains supported auth
        // methods that server can use to proceed with further negotiation.
        let auth_request = AuthMethodRequest::from(&mut self.stream).await?;

        Ok(())
    }
}
