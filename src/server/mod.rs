use self::peer::handlers::LurkSocks5ClientHandler;
use crate::{
    io::stream::LurkStreamWrapper,
    server::peer::{auth::LurkAuthenticator, LurkPeerType, LurkTcpPeer},
};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

mod peer;

pub mod config;

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
                Ok((stream, addr)) => self.on_new_peer_connected(stream, addr).await,
                Err(err) => warn!("Error while accepting the TCP connection: {}", err),
            }
        }
    }

    async fn bind(&self) -> Result<TcpListener> {
        let tcp_listener = TcpListener::bind(self.addr).await?;
        info!("Listening on {}", self.addr);

        Ok(tcp_listener)
    }

    async fn on_new_peer_connected(&self, stream: TcpStream, addr: SocketAddr) {
        info!("New connection has been established from {}", addr);

        // Identify peer type.
        let peer_type = match LurkPeerType::from_tcp_stream(&stream).await {
            Ok(t) => t,
            Err(err) => {
                error!(
                    "Failed to identify connected {addr} peer type \
                        with error '{err}'. Skip connection."
                );
                return;
            }
        };

        debug!("Connected {addr} peer type {peer_type:?}");

        // Wrap incoming stream and create peer instance.
        let stream_wrapper = LurkStreamWrapper::new(stream);
        let mut peer = LurkTcpPeer::new(stream_wrapper, addr, peer_type);

        // Create connection handler and supply handling of new peer in a separate thread.
        let mut handler = LurkConnectionHandler {
            server_address: self.addr,
            authenticator: LurkAuthenticator::new(self.auth_enabled),
        };

        tokio::spawn(async move {
            if let Err(err) = handler.handle_peer(&mut peer).await {
                error!("Error occured during handling of {}, {}", addr, err);
            }
            info!("Connection with {} has been finished", peer);
        });
    }
}

struct LurkConnectionHandler {
    server_address: SocketAddr,
    authenticator: LurkAuthenticator,
}

impl LurkConnectionHandler {
    async fn handle_peer(&mut self, peer: &mut LurkTcpPeer) -> Result<()> {
        match peer.peer_type() {
            LurkPeerType::Socks5Peer => {
                let mut socks5_handler = LurkSocks5ClientHandler::new(peer, &mut self.authenticator, self.server_address);
                socks5_handler.handle_peer().await
            }
        }
    }
}

#[cfg(test)]
mod tests {}
