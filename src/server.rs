use crate::{
    auth::LurkAuthenticator,
    client::LurkClient,
    proto::socks5::{AuthMethod, HandshakeResponse},
};
use anyhow::Result;
use log::{debug, error, info, trace, warn};
use std::{
    collections::HashSet,
    net::{Ipv4Addr, SocketAddr},
};
use tokio::net::{TcpListener, TcpStream};

pub struct LurkServer {
    ip: Ipv4Addr,
    port: u16,
    auth_enabled: bool,
}

impl LurkServer {
    pub fn new(ip: Ipv4Addr, port: u16, auth_enabled: bool) -> LurkServer {
        LurkServer { ip, port, auth_enabled }
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
        let tcp_listener = TcpListener::bind((self.ip, self.port)).await?;
        info!("Listening on {}:{}", self.ip, self.port);
        Ok(tcp_listener)
    }

    fn on_client_connected(&self, stream: TcpStream, addr: SocketAddr) {
        info!("New connection has been established from {}", addr);
        let mut client = LurkClient::new(stream, addr);
        let handler = LurkConnectionHandler {
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
        let handshake_response = HandshakeResponse::new(selected_method);
        client.write_handshake_response(&handshake_response).await?;

        // Complete authentication.
        LurkAuthenticator::authenticate(client, selected_method);

        // Handle relay request
        let relay_request = client.read_relay_request().await?;

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
