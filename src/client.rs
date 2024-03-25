use anyhow::{Ok, Result};
use log::trace;
use std::net::SocketAddr;
use tokio::net::TcpStream;

use crate::proto::{
    message::LurkMessageHandler,
    socks5::{Address, AuthMethod, HandshakeRequest, HandshakeResponse, RelayRequest, RelayResponse, ReplyStatus},
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
        let request: HandshakeRequest = LurkMessageHandler::read_request(&mut self.stream).await?;
        Ok(request)
    }

    /// Handle traffic relay request from client. Expected to be sent from client right
    /// after authentication phase.
    pub async fn read_relay_request(&mut self) -> Result<RelayRequest> {
        let request: RelayRequest = LurkMessageHandler::read_request(&mut self.stream).await?;
        Ok(request)
    }

    /// Writes response to RelayRequest
    pub async fn write_handshake_response(&mut self, selected_method: AuthMethod) -> Result<()> {
        let response = HandshakeResponse::new(selected_method);
        LurkMessageHandler::write_response(&mut self.stream, response).await?;
        Ok(())
    }

    pub async fn write_relay_response(&mut self, bound_addr: SocketAddr, status: ReplyStatus) -> Result<()> {
        let response = RelayResponse::new(Address::SocketAddress(bound_addr), status);
        LurkMessageHandler::write_response(&mut self.stream, response).await?;
        Ok(())
    }
}
