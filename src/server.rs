use anyhow::Result;
use log::{info, warn};
use std::net::Ipv4Addr;
use tokio::net::TcpListener;

pub struct LurkServer {
    tcp_listener: TcpListener,
}

impl LurkServer {
    pub async fn new(ipv4: Ipv4Addr, port: u16) -> Result<LurkServer> {
        let tcp_listener = TcpListener::bind((ipv4, port)).await?;
        info!("Listening on {:}:{:}", ipv4, port);

        Ok(LurkServer { tcp_listener })
    }

    pub async fn run(&self) {
        loop {
            match self.tcp_listener.accept().await {
                Ok((_stream, addr)) => {
                    info!("new client: {:?}", addr);
                }
                Err(e) => warn!("couldn't get client: {:?}", e),
            }
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
