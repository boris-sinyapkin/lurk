use crate::{
    auth::LurkAuthenticator,
    client::LurkClient,
    proto::socks5::{Address, Command, ReplyStatus},
};
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::net::{TcpListener, TcpStream};

pub struct LurkServer {
    addr: SocketAddr,
    auth_enabled: bool,
}

impl LurkServer {
    pub fn new(ip: Ipv4Addr, port: u16, auth_enabled: bool) -> LurkServer {
        LurkServer {
            addr: SocketAddr::V4(SocketAddrV4::new(ip, port)),
            auth_enabled,
        }
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
        let mut client = LurkClient::new(stream, addr, self.auth_enabled);
        let handler = LurkConnectionHandler { server_addr: self.addr };
        tokio::spawn(async move {
            if let Err(err) = handler.handle_client(&mut client).await {
                error!("Error occured during handling of client {}, {}", addr, err);
            }
        });
    }
}

struct LurkConnectionHandler {
    server_addr: SocketAddr,
}

impl LurkConnectionHandler {
    async fn handle_client(&self, client: &mut LurkClient) -> Result<()> {
        let auth_method = client.handshake().await?;
        debug!("Selected auth method '{:?}' for {}", auth_method, client);

        // Authenticate client with selected method.
        LurkAuthenticator::authenticate(client, auth_method);

        // Read incoming relay request
        let relay_request = client.read_relay_request().await?;
        let target = relay_request.target_addr();

        match relay_request.command() {
            Command::Connect => self.handle_connect(client, target).await,
            Command::Bind => todo!(),
            Command::UdpAssociate => todo!(),
        }
    }

    async fn handle_connect(&self, client: &mut LurkClient, target: &Address) -> Result<()> {
        debug!("Handling SOCKS5 CONNECT from {}", client);
        // Establish TCP connection with the target host.
        let mut target_stream = match LurkConnectionHandler::establish_tcp_connection(target).await {
            Ok(stream) => {
                debug!("TCP connection has been established with the target {:?}", target);
                client
                    .respond_to_relay_request(self.server_addr, ReplyStatus::Succeeded)
                    .await?;
                stream
            }
            Err(_) => todo!(),
        };

        // Start data relaying.
        client.relay_data(&mut target_stream).await;

        Ok(())
    }

    async fn establish_tcp_connection(target: &Address) -> Result<TcpStream> {
        let tcp_stream = match target {
            Address::SocketAddress(sock_addr) => TcpStream::connect(sock_addr).await?,
            Address::DomainName(_, _) => return Err(anyhow!("Domains are not supported")),
        };

        Ok(tcp_stream)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn empty_test() {
        print!("Empty test");
    }
}
