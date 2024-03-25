use anyhow::{Ok, Result};
use log::trace;
use std::net::SocketAddr;
use tokio::net::TcpStream;

use crate::proto::{
    message::LurkRequest,
    message::LurkResponse,
    socks5::{HandshakeRequest, HandshakeResponse, RelayRequest},
};

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

    /// Handle "handshake" request. According to SOCKS5 protocol definition, it contains
    /// supported by client auth methods that server can use to proceed with further negotiation.
    pub async fn read_handshake_request(&mut self) -> Result<HandshakeRequest> {
        let request = HandshakeRequest::read_from(&mut self.stream).await?;
        trace!("Read {} from {}", request, self.addr());
        Ok(request)
    }

    /// Handle traffic relay request from client. Expected to be sent from client right after authentication phase.
    pub async fn read_relay_request(&mut self) -> Result<RelayRequest> {
        let request = RelayRequest::read_from(&mut self.stream).await?;
        trace!("Read {} from {}", request, self.addr());
        Ok(request)
    }

    /// Writes response to RelayRequest
    pub async fn write_handshake_response(&mut self, response: &HandshakeResponse) -> Result<()> {
        response.write_to(&mut self.stream).await?;
        trace!("Write {} to {}", response, self.addr());
        Ok(())
    }
}
