use anyhow::{anyhow, Result};
use log::{debug, trace};
use std::{
    fmt::Display,
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::TcpStream,
};

use crate::{
    auth::LurkAuthenticator,
    error::LurkError,
    proto::{
        message::{LurkRequestReader, LurkResponseWriter, LurkStreamWrapper},
        socks5::{Address, AuthMethod, HandshakeRequest, HandshakeResponse, RelayRequest, RelayResponse, ReplyStatus},
    },
};

pub struct LurkClient<S>
where
    S: LurkRequestReader + LurkResponseWriter + Unpin,
{
    addr: SocketAddr,
    stream: S,
}

pub type LurkTcpClient = LurkClient<LurkStreamWrapper<TcpStream>>;

impl<S> LurkClient<S>
where
    S: LurkRequestReader + LurkResponseWriter + Unpin + Deref + DerefMut,
{
    pub fn new(stream: S, addr: SocketAddr) -> LurkClient<S> {
        LurkClient { stream, addr }
    }

    /// Handshaking with client.
    /// On success, return established authentication method.
    pub async fn handshake(&mut self, authenticator: &LurkAuthenticator) -> Result<AuthMethod> {
        // Obtain client authentication methods from SOCKS5 hanshake message.
        let handshake_request = self.stream.read_request::<HandshakeRequest>().await?;
        let client_methods = handshake_request.auth_methods();
        // Choose authentication method.
        let method = authenticator.select_auth_method(client_methods);
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

    pub async fn relay_data<T>(&mut self, target_stream: &mut T)
    where
        T: AsyncRead + AsyncWrite + Unpin,
        <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("Starting data relaying tunnel for {} ...", self);
        match copy_bidirectional(&mut *self.stream, target_stream).await {
            Ok((l2r, r2l)) => trace!("tunnel closed, L2R {} bytes, R2L {} bytes transmitted", l2r, r2l),
            Err(err) => trace!("tunnel closed with error: {}", err),
        }
    }
}

impl<S> Display for LurkClient<S>
where
    S: LurkRequestReader + LurkResponseWriter + Unpin,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client {}", self.addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_none_auth_method() {
        /*
         * let stream = MockStream::new();
         * let client = MockClient::new();
         * let authenticator = MockAuthenticator::new()
         *
         * stream.expect
         *
         *
         * client.handshake(authenticator).await?
         *
         */
    }
}
