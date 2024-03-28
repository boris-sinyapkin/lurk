use anyhow::{anyhow, Result};
use log::{debug, trace};
use std::{collections::HashSet, fmt::Display, net::SocketAddr};
use tokio::{
    io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt}, net::TcpStream,
};

use crate::{
    auth::LurkAuthenticator,
    error::LurkError,
    proto::{
        message::LurkStreamWrapper,
        socks5::{Address, AuthMethod, HandshakeRequest, HandshakeResponse, RelayRequest, RelayResponse, ReplyStatus},
    },
};

pub struct LurkClient<S: AsyncReadExt + AsyncWriteExt + Unpin> {
    addr: SocketAddr,
    auth_enabled: bool,
    stream: LurkStreamWrapper<S>,
}

pub type LurkTcpClient = LurkClient<TcpStream>;

impl<S> LurkClient<S>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    pub fn new(stream: S, addr: SocketAddr, auth_enabled: bool) -> LurkClient<S> {
        LurkClient {
            stream: LurkStreamWrapper::new(stream),
            addr,
            auth_enabled,
        }
    }

    /// Handshaking with client.
    /// On success, return established authentication method.
    pub async fn handshake(&mut self) -> Result<AuthMethod> {
        // Obtain client authentication methods from SOCKS5 hanshake message.
        let handshake_request = self.stream.read_request::<HandshakeRequest>().await?;
        // Choose authentication method.
        let method = self.select_auth_method(handshake_request.auth_methods());
        // Respond to handshake request.
        let response = HandshakeResponse::new(method);
        self.stream.write_response(response).await?;

        method.ok_or(anyhow!(LurkError::NoAcceptableAuthMethod(self.addr)))
    }

    pub async fn read_relay_request(&mut self) -> Result<RelayRequest> {
        self.stream.read_request::<RelayRequest>().await
    }

    pub async fn respond_to_relay_request(&mut self, server_addr: SocketAddr, status: ReplyStatus) -> Result<()> {
        let response = RelayResponse::new(Address::SocketAddress(server_addr), status);
        self.stream.write_response(response).await
    }

    pub async fn relay_data(&mut self, target_stream: &mut S) {
        debug!("Starting data relaying tunnel for {} ...", self);
        match copy_bidirectional(&mut *self.stream, target_stream).await {
            Ok((l2r, r2l)) => trace!("tunnel closed, L2R {} bytes, R2L {} bytes transmitted", l2r, r2l),
            Err(err) => trace!("tunnel closed with error: {}", err),
        }
    }

    fn select_auth_method(&self, client_methods: &HashSet<AuthMethod>) -> Option<AuthMethod> {
        // Found intersection between available auth methods on server and supported methods by client.
        let server_methods = LurkAuthenticator::available_methods();
        let common_methods = server_methods
            .intersection(client_methods)
            .collect::<HashSet<&AuthMethod>>();

        if !self.auth_enabled && common_methods.contains(&AuthMethod::None) {
            return Some(AuthMethod::None);
        }

        None
    }
}

impl<S> Display for LurkClient<S>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client {}", self.addr)
    }
}
