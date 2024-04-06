use crate::{auth::LurkAuthenticator, client::LurkTcpClient, io::stream::LurkStreamWrapper};
use anyhow::Result;
use log::{error, info, warn};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

pub struct LurkServer {
    addr: SocketAddr,
    auth_enabled: bool,
}

impl LurkServer {
    pub fn new(addr: SocketAddr, auth_enabled: bool) -> LurkServer {
        LurkServer { addr, auth_enabled }
    }

    pub async fn run(&self) -> Result<()> {
        if !self.auth_enabled {
            warn!("Authentication disabled");
        }
        let tcp_listener = self.bind().await?;
        loop {
            match tcp_listener.accept().await {
                Ok((stream, addr)) => self.on_client_connected(stream, addr),
                Err(err) => {
                    warn!("Error while accepting the TCP connection: {}", err);
                    continue;
                }
            }
        }
    }

    async fn bind(&self) -> Result<TcpListener> {
        let tcp_listener = TcpListener::bind(self.addr).await?;
        info!("Listening on {}", self.addr);
        Ok(tcp_listener)
    }

    fn on_client_connected(&self, stream: TcpStream, addr: SocketAddr) {
        info!("New connection has been established from {}", addr);
        let mut client = LurkTcpClient::new(LurkStreamWrapper::new(stream), addr);
        let mut handler = LurkConnectionHandler {
            server_addr: self.addr,
            authenticator: LurkAuthenticator::new(self.auth_enabled),
        };
        tokio::spawn(async move {
            if let Err(err) = handler.handle_socks5_client(&mut client).await {
                error!("Error occured during handling of client {}, {}", addr, err);
            }

            info!("Connection with {} has been finished", client);
        });
    }
}

struct LurkConnectionHandler {
    server_addr: SocketAddr,
    authenticator: LurkAuthenticator,
}

impl LurkConnectionHandler {
    async fn handle_socks5_client(&mut self, client: &mut LurkTcpClient) -> Result<()> {
        // Complete handshake process and negotiate the authentication method.
        client.process_socks5_handshake(&mut self.authenticator).await?;

        // Authenticate client with selected method.
        self.authenticator.authenticate(client);

        // Proceed with SOCKS5 relay handling.
        // This will receive and process relay request, handle SOCKS5 command
        // and establish the tunnel "client <-- lurk proxy --> target".
        client.process_socks5_command(self.server_addr).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {}
