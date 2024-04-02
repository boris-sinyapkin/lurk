use crate::{
    auth::LurkAuthenticator,
    error::LurkError,
    io::{stream::LurkStreamWrapper, LurkRequestRead, LurkResponseWrite},
    proto::socks5::{
        request::{HandshakeRequest, RelayRequest},
        response::{HandshakeResponse, RelayResponse},
        Address, AuthMethod, ReplyStatus,
    },
};
use anyhow::{anyhow, bail, Result};
use log::{debug, error};
use std::{
    fmt::Display,
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub struct LurkClient<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin,
{
    addr: SocketAddr,
    stream: S,
}

pub type LurkTcpClient = LurkClient<LurkStreamWrapper<TcpStream>>;

impl<S> LurkClient<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin + DerefMut,
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

    pub async fn relay_data<T>(&mut self, target_stream: &mut T) -> Result<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
        <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
    {
        match copy_bidirectional(&mut *self.stream, target_stream).await {
            Ok((l2r, r2l)) => debug!("Tunnel closed, L2R {} bytes, R2L {} bytes transmitted", l2r, r2l),
            Err(err) => {
                error!("Tunnel closed with error: {}", err);
                bail!(err)
            }
        }
        Ok(())
    }
}

impl<S> Display for LurkClient<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::stream::MockLurkStreamWrapper;
    use mockall::predicate;
    use std::{
        collections::HashSet,
        net::{IpAddr, Ipv4Addr},
    };
    use tokio_test::io::Mock;

    #[tokio::test]
    async fn socks5_handshake() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut stream = MockLurkStreamWrapper::<Mock>::new();

        let client_methods = [AuthMethod::None, AuthMethod::GssAPI];
        let agreed_method = AuthMethod::None;

        stream
            .expect_read_request()
            .once()
            .returning(move || Ok(HandshakeRequest::new(HashSet::from(client_methods))));

        stream
            .expect_write_response()
            .once()
            .with(predicate::eq(HandshakeResponse::new(Some(agreed_method))))
            .returning(|_| Ok(()));

        let mut client = LurkClient::new(stream, addr);
        let authenticator = LurkAuthenticator::new(false);

        assert_eq!(agreed_method, client.handshake(&authenticator).await.unwrap());
    }
}
