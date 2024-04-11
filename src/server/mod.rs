use self::peer::handlers::LurkSocks5PeerHandler;
use crate::{
    io::stream::LurkStreamWrapper,
    server::peer::{LurkPeerType, LurkTcpPeer},
};
use anyhow::Result;
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

mod peer;

pub mod config;

pub struct LurkServer {
    addr: SocketAddr,
}

impl LurkServer {
    pub fn new(addr: SocketAddr) -> LurkServer {
        LurkServer { addr }
    }

    pub async fn run(&self) -> Result<()> {
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
            Ok(t) => {
                debug!("Connected {addr} peer type {t:?}");
                t
            }
            Err(err) => {
                error!(
                    "Failed to identify connected {addr} peer type \
                        with error '{err}'. Skip connection."
                );
                return;
            }
        };

        // Wrap incoming stream and create peer instance.
        let stream_wrapper = LurkStreamWrapper::new(stream);
        let mut peer = LurkTcpPeer::new(stream_wrapper, addr, peer_type);

        // Create connection handler and supply handling of new peer in a separate thread.
        let mut handler = LurkConnectionHandler { server_address: self.addr };

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
}

impl LurkConnectionHandler {
    async fn handle_peer(&mut self, peer: &mut LurkTcpPeer) -> Result<()> {
        match peer.peer_type() {
            LurkPeerType::Socks5Peer => {
                let mut socks5_handler = LurkSocks5PeerHandler::new(peer, self.server_address);

                socks5_handler.handle().await
            }
        }
    }
}

#[cfg(test)]
mod tests {}
