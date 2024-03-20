use crate::connection::LurkConnection;
use anyhow::Result;
use log::{debug, error, info, warn};
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::{TcpListener, TcpStream};

pub struct LurkServer {
    tcp_listener: TcpListener,
}

impl LurkServer {
    pub async fn new(ipv4: Ipv4Addr, port: u16) -> Result<LurkServer> {
        let tcp_listener = TcpListener::bind((ipv4, port)).await?;
        info!("Listening on {}:{}", ipv4, port);

        Ok(LurkServer { tcp_listener })
    }

    pub async fn run(&self) {
        loop {
            match self.tcp_listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New client connected from {}", addr);
                    tokio::spawn(async move {
                        LurkServer::on_client_connected(stream, addr).await
                    });
                }
                Err(err) => {
                    warn!("Error while accepting the TCP connection: {}", err);
                    continue;
                }
            }
        }
    }

    async fn on_client_connected(stream: TcpStream, addr: SocketAddr) {
        // Create new connection with incoming stream.
        let mut connection = LurkConnection::new(stream);

        if let Err(err) = connection.iterate().await {
            error!("Error occured during connection handling (source address: {}): {}", addr, err);
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
