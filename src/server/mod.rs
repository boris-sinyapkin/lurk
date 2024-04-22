use self::peer::handlers::LurkSocks5PeerHandler;
use crate::{
    common::logging::{log_closed_tcp_conn, log_closed_tcp_conn_with_error, log_opened_tcp_conn},
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
                Ok((stream, addr)) => {
                    log_opened_tcp_conn!(addr);
                    self.on_new_peer_connected(stream, addr).await
                }
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
        // Identify peer type.
        let peer_type = match LurkPeerType::from_tcp_stream(&stream).await {
            Ok(t) => {
                debug!("Connected {addr} peer type {t}");
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
                log_closed_tcp_conn_with_error!(peer, err);
            } else {
                log_closed_tcp_conn!(peer);
            }
        });
    }
}

struct LurkConnectionHandler {
    server_address: SocketAddr,
}

impl LurkConnectionHandler {
    async fn handle_peer(&mut self, peer: &mut LurkTcpPeer) -> Result<()> {
        match peer.peer_type() {
            LurkPeerType::SOCKS5 => {
                let mut socks5_handler = LurkSocks5PeerHandler::new(peer, self.server_address);

                socks5_handler.handle().await
            }
        }
    }
}

#[cfg(test)]
mod tests {}
