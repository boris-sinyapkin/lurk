use anyhow::{Ok, Result};
use std::net::SocketAddr;
use tokio::net::TcpStream;

use crate::proto::socks5::{AuthMethod, HandshakeRequest, HandshakeResponse};

pub struct LurkClient {
    addr: SocketAddr,
    stream: TcpStream,
}

impl LurkClient {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> LurkClient {
        LurkClient { stream, addr }
    }

    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub async fn read_handshake_request(&mut self) -> Result<HandshakeRequest> {
        // Handle "handshake" request. According to SOCKS5 protocol definition, it contains
        // supported by client auth methods that server can use to proceed with further negotiation.
        HandshakeRequest::parse_from(&mut self.stream).await
    }

    pub async fn write_handshake_response(&mut self, response: HandshakeResponse) -> Result<()> {
        Ok(response.write_to(&mut self.stream).await?)
    }
}
