use crate::{
    auth::LurkAuthenticator,
    client::LurkClient,
    proto::socks5::{self, Address, AuthMethod, Command, ReplyStatus},
};
use anyhow::Result;
use log::{debug, error, info, trace, warn};
use std::{
    collections::HashSet,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};
use tokio::net::{TcpListener, TcpStream};

pub struct LurkServer {
    addr: SocketAddr,
    auth_enabled: bool,
}

impl LurkServer {
    pub fn new(ip: Ipv4Addr, port: u16, auth_enabled: bool) -> LurkServer {
        LurkServer { 
            addr: SocketAddr::V4(SocketAddrV4::new(ip, port)),
            auth_enabled
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
        let mut client = LurkClient::new(stream, addr);
        let handler = LurkConnectionHandler {
            bound_addr: self.addr,
            auth_enabled: self.auth_enabled,
        };
        tokio::spawn(async move {
            if let Err(err) = handler.handle_client(&mut client).await {
                error!("Error occured during handling of client {}, {}", addr, err);
            }
        });
    }
}

struct LurkConnectionHandler {
    bound_addr: SocketAddr,
    auth_enabled: bool,
}

impl LurkConnectionHandler {
    async fn handle_client(&self, client: &mut LurkClient) -> Result<()> {
        // Obtain client authentication methods from SOCKS5 hanshake message.
        let handshake_request = client.read_handshake_request().await?;

        // Choose authentication method.
        let selected_method = self.select_auth_method(handshake_request.auth_methods())?;
        debug!(
            "Selected auth method '{:?}' for client {}",
            selected_method,
            client.addr()
        );

        // Tell chosen method to client.
        client.write_handshake_response(selected_method).await?;

        // Authenticate client with selected method.
        LurkAuthenticator::authenticate(client, selected_method);

        // Handle relay request
        let relay_request = client.read_relay_request().await?;

        match relay_request.command() {
            Command::Connect => self.handle_connect(client, relay_request.dest_addr()).await?,
            Command::Bind => todo!(),
            Command::UdpAssociate => todo!(),
        }

        Ok(())
    }

    async fn handle_connect(&self, client: &mut LurkClient, dest_addr: &Address) -> Result<()> {
        let stream = match dest_addr {
            Address::SocketAddress(sock_addr) => TcpStream::connect(sock_addr).await?,
            Address::DomainName(_, _) => todo!(),
        };

        debug!("TCP connection has been established with the destination {:?}", dest_addr);
        client.write_relay_response(self.bound_addr, ReplyStatus::Succeeded).await?;

        Ok(())
    }

    fn select_auth_method(&self, client_methods: &HashSet<AuthMethod>) -> Result<AuthMethod> {
        // Found intersection between available auth methods on server and supported methods by client.
        let server_methods = LurkAuthenticator::available_methods();
        let common_methods = server_methods
            .intersection(client_methods)
            .collect::<HashSet<&AuthMethod>>();

        // Proceed without auth if it's disabled externaly.
        if !self.auth_enabled {
            if common_methods.contains(&AuthMethod::None) {
                Ok(AuthMethod::None)
            } else {
                todo!()
            }
        } else {
            todo!()
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn empty_test() {
        print!("Empty test");
    }
}
