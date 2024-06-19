use self::handlers::LurkSocks5Handler;
use crate::{
    common::logging::{self},
    net::tcp::{
        connection::{LurkTcpConnection, LurkTcpConnectionLabel},
        listener::LurkTcpListener,
    },
};
use anyhow::Result;
use async_listen::is_transient_error;
use log::{error, info, warn};
use std::{net::SocketAddr, time::Duration};
use tokio::time::sleep;

mod handlers;

pub struct LurkServer {
    bind_addr: SocketAddr,
    conn_limit: usize,
}

impl LurkServer {
    /// Delay after non-transient TCP acception failure, e.g.
    /// handle resource exhaustion errors.
    const DELAY_AFTER_ERROR_MILLIS: u64 = 500;

    pub fn new(bind_addr: SocketAddr, conn_limit: usize) -> LurkServer {
        LurkServer { bind_addr, conn_limit }
    }

    pub async fn run(&self) -> Result<()> {
        let mut tcp_listener = LurkTcpListener::bind(self.bind_addr, self.conn_limit).await?;
        info!("Listening on {} (TCP connections limit {})", self.bind_addr, self.conn_limit);

        loop {
            match tcp_listener.accept().await {
                Ok(conn) => self.on_tcp_connection_established(conn).await,
                Err(err) => self.on_tcp_acception_error(err).await,
            }
        }
    }

    async fn on_tcp_acception_error(&self, err: anyhow::Error) {
        logging::log_tcp_acception_error!(err);

        if let Some(err) = err.downcast_ref::<std::io::Error>() {
            if !is_transient_error(err) {
                // Perform sleep after non-transient errors
                sleep(Duration::from_millis(LurkServer::DELAY_AFTER_ERROR_MILLIS)).await;
            }
        }
    }

    async fn on_tcp_connection_established(&self, conn: LurkTcpConnection) {
        let (conn_peer_addr, conn_label) = (conn.peer_addr(), conn.label());
        logging::log_tcp_established_conn!(conn_peer_addr, conn_label);

        // Create connection handler and supply handling of particular traffic label in a separate thread.
        let mut connection_handler = match conn.label() {
            LurkTcpConnectionLabel::SOCKS5 => LurkSocks5Handler::new(conn),
            unknown_label => {
                logging::log_tcp_closed_conn_with_error!(conn_peer_addr, conn_label, unknown_label);
                return;
            }
        };

        tokio::spawn(async move {
            if let Err(err) = connection_handler.handle().await {
                logging::log_tcp_closed_conn_with_error!(conn_peer_addr, conn_label, err);
            } else {
                logging::log_tcp_closed_conn!(conn_peer_addr, conn_label);
            }
        });
    }
}

#[cfg(test)]
mod tests {}
