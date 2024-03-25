use anyhow::Result;
use log::{debug, trace};
use std::net::SocketAddr;
use tokio::{io::copy_bidirectional, net::TcpStream};

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

    pub async fn relay(&mut self, dest_stream: &mut TcpStream) {
        match copy_bidirectional(&mut self.stream, dest_stream).await {
            Ok((rn, wn)) => trace!("(bypassed) closed, L2R {} bytes, R2L {} bytes", rn, wn),
            Err(err) => trace!("closed with error: {}", err),
        }
    }

    /// Handle "handshake" request. According to SOCKS5 protocol definition, it contains
    /// supported by client auth methods that server can use to proceed with further negotiation.
    pub async fn read_handshake_request(&mut self) -> Result<HandshakeRequest> {
        LurkMessageHandler::read_request::<_, HandshakeRequest>(&mut self.stream).await
    }

    /// Handle traffic relay request from client. Expected to be sent from client right
    /// after authentication phase.
    pub async fn read_relay_request(&mut self) -> Result<RelayRequest> {
        LurkMessageHandler::read_request::<_, RelayRequest>(&mut self.stream).await
    }

    /// Writes response to RelayRequest
    pub async fn write_handshake_response(&mut self, selected_method: AuthMethod) -> Result<()> {
        let response = HandshakeResponse::new(selected_method);
        LurkMessageHandler::write_response(&mut self.stream, response).await
    }

    /// Writes response to ReplyResponse
    pub async fn write_relay_response(&mut self, bound_addr: SocketAddr, status: ReplyStatus) -> Result<()> {
        let response = RelayResponse::new(Address::SocketAddress(bound_addr), status);
        LurkMessageHandler::write_response(&mut self.stream, response).await
    }
}
