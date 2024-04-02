use crate::{
    auth::LurkAuthenticator,
    client::LurkTcpClient,
    error::{LurkError, Unsupported},
    io::stream::LurkStreamWrapper,
    proto::socks5::{Address, Command, ReplyStatus},
};
use anyhow::{bail, Result};
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use tokio::net::{lookup_host, TcpListener, TcpStream};

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
        let handler = LurkConnectionHandler {
            server_addr: self.addr,
            authenticator: LurkAuthenticator::new(self.auth_enabled),
        };
        tokio::spawn(async move {
            if let Err(err) = handler.handle_client(&mut client).await {
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
    async fn handle_client(&self, client: &mut LurkTcpClient) -> Result<()> {
        // Complete handshake process and negotiate the authentication method.
        let auth_method = client.handshake(&self.authenticator).await?;
        debug!("Selected auth method '{:?}' for {}", auth_method, client);

        // Authenticate client with selected method.
        self.authenticator.authenticate(client, auth_method);

        // Proceed with SOCKS5 relay handling.
        // This will receive and process relay request, handle SOCKS5 command
        // and establish the tunnel "client <-- lurk proxy --> target".
        if let Err(err) = self.handle_relay(client).await {
            self.on_handle_relay_error(client, err).await?
        }

        Ok(())
    }

    async fn handle_relay(&self, client: &mut LurkTcpClient) -> Result<()> {
        let relay_req = client.read_relay_request().await?;

        match relay_req.command() {
            Command::Connect => self.handle_connect_command(client, relay_req.target_addr()).await,
            _ => bail!(LurkError::Unsupported(Unsupported::Socks5Command(relay_req.command()))),
        }
    }

    async fn handle_connect_command(&self, client: &mut LurkTcpClient, target: &Address) -> Result<()> {
        debug!("Handling SOCKS5 CONNECT from {}", client);

        // Establish TCP connection with the target host.
        let mut target_stream = LurkConnectionHandler::establish_tcp_connection(target).await?;

        debug!("TCP connection has been established with the target {}", target);
        client
            .respond_to_relay_request(self.server_addr, ReplyStatus::Succeeded)
            .await?;

        debug!(
            "Starting data relaying tunnel: client [{}] <---> lurk [{}] <---> destination [{}]",
            client, self.server_addr, target
        );
        client.relay_data(&mut target_stream).await?;

        Ok(())
    }

    async fn establish_tcp_connection(target: &Address) -> Result<TcpStream> {
        let addr = match target {
            Address::SocketAddress(sock_addr) => *sock_addr,
            Address::DomainName(hostname, port) => {
                // Resolve by means of builtin tokio DNS resolver
                let resolved_names = lookup_host(format!("{hostname:}:{port:}")).await?;
                // Take first found
                resolved_names
                    .into_iter()
                    .nth(0)
                    .ok_or(LurkError::UnresolvedDomainName(hostname.to_string()))?
            }
        };

        TcpStream::connect(addr).await.map_err(anyhow::Error::from)
    }

    async fn on_handle_relay_error(&self, client: &mut LurkTcpClient, err: anyhow::Error) -> Result<()> {
        let error = err.to_string();
        let status = ReplyStatus::from(err);
        debug!("Error: '{}'. Replied with status '{:?}' to {}", error, status, client);
        client.respond_to_relay_request(self.server_addr, status).await
    }
}

#[cfg(test)]
mod tests {}
