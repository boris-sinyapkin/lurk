use anyhow::Result;
use log::trace;
use std::{fmt::Debug, net::SocketAddr};
use tokio::{io::copy_bidirectional, net::TcpStream};

use crate::proto::{
    message::{LurkMessageHandler, LurkResponse},
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
        self.write_response_to_stream(HandshakeResponse::new(selected_method))
            .await
    }

    /// Writes response to ReplyResponse
    pub async fn write_relay_response(&mut self, bound_addr: SocketAddr, status: ReplyStatus) -> Result<()> {
        self.write_response_to_stream(RelayResponse::new(Address::SocketAddress(bound_addr), status))
            .await
    }

    async fn write_response_to_stream<R: LurkResponse + Debug>(&mut self, response: R) -> Result<()> {
        LurkMessageHandler::write_response(&mut self.stream, response).await?;
        Ok(())
    }
}
