use crate::{
    common::logging::{self},
    net::tcp::{connection::LurkTcpConnection, listener::LurkTcpListener},
};
use anyhow::Result;
use async_listen::is_transient_error;
use handlers::create_tcp_connection_handler;
use log::{error, info, warn};
use stats::LurkServerStats;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::time::sleep;

mod handlers;

pub mod stats;

pub struct LurkServer {
    bind_addr: SocketAddr,
    stats: Arc<LurkServerStats>,
}

impl LurkServer {
    /// Delay after non-transient TCP acception failure, e.g.
    /// handle resource exhaustion errors.
    const DELAY_AFTER_ERROR_MILLIS: u64 = 500;

    pub fn new(bind_addr: SocketAddr) -> LurkServer {
        LurkServer {
            bind_addr,
            stats: Arc::new(LurkServerStats::new()),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let mut tcp_listener = LurkTcpListener::bind(self.bind_addr).await?;
        info!("Proxy is listening on {}", self.bind_addr);

        self.stats.on_server_started();

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
        let mut connection_handler = match create_tcp_connection_handler(&conn.label()) {
            Ok(handler) => handler,
            Err(err) => {
                logging::log_tcp_closed_conn_with_error!(conn_peer_addr, conn_label, err);
                return;
            }
        };

        // Submit execution in a separate task.
        tokio::spawn(async move {
            if let Err(err) = connection_handler.handle(conn).await {
                logging::log_tcp_closed_conn_with_error!(conn_peer_addr, conn_label, err);
            } else {
                logging::log_tcp_closed_conn!(conn_peer_addr, conn_label);
            }
        });
    }

    pub fn get_stats(&self) -> Arc<LurkServerStats> {
        Arc::clone(&self.stats)
    }
}

#[cfg(test)]
mod tests {}
