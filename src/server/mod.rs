use self::peer::handlers::LurkSocks5PeerHandler;
use crate::{
    common::logging::{
        log_closed_tcp_conn, log_closed_tcp_conn_with_error, log_failed_tcp_conn_acception, log_opened_tcp_conn, log_skipped_tcp_conn,
    },
    io::stream::LurkStreamWrapper,
    server::peer::{LurkPeerType, LurkTcpPeer},
};
use anyhow::Result;
use log::{error, info, warn};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    time::sleep,
};

mod peer;

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
                Ok((stream, addr)) => self.on_tcp_connection_established(stream, addr).await,
                Err(err) => {
                    log_failed_tcp_conn_acception!(err);
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    async fn bind(&self) -> Result<TcpListener> {
        let tcp_listener = TcpListener::bind(self.addr).await?;
        info!("Listening on {}", self.addr);

        Ok(tcp_listener)
    }

    async fn on_tcp_connection_established(&self, stream: TcpStream, addr: SocketAddr) {
        // Identify peer type.
        let peer_type = match LurkPeerType::from_tcp_stream(&stream).await {
            Ok(t) => t,
            Err(err) => {
                log_skipped_tcp_conn!(addr, err);
                return;
            }
        };

        log_opened_tcp_conn!(addr, peer_type);

        // Wrap incoming stream and create peer instance.
        let stream_wrapper = LurkStreamWrapper::new(stream);
        let peer = LurkTcpPeer::new(stream_wrapper, addr);

        // Create connection handler and supply handling of new peer in a separate thread.
        let mut peer_handler = match peer_type {
            LurkPeerType::SOCKS5 => LurkSocks5PeerHandler::new(peer, self.addr),
        };

        tokio::spawn(async move {
            if let Err(err) = peer_handler.handle().await {
                log_closed_tcp_conn_with_error!(addr, err);
            } else {
                log_closed_tcp_conn!(addr);
            }
        });
    }
}

#[cfg(test)]
mod tests {}
