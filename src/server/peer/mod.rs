use self::{auth::LurkAuthenticator, handlers::LurkSocks5RequestHandler};
use crate::{
    io::{stream::LurkStreamWrapper, LurkRequestRead, LurkResponseWrite},
    proto::socks5::request::{HandshakeRequest, RelayRequest},
};
use anyhow::Result;
use std::{
    fmt::Display,
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub mod auth;

mod handlers;

pub type LurkTcpPeer = LurkPeer<LurkStreamWrapper<TcpStream>>;

pub struct LurkPeer<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin,
{
    addr: SocketAddr,
    stream: S,
}

impl<S> LurkPeer<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin + DerefMut,
    <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S, addr: SocketAddr) -> LurkPeer<S> {
        LurkPeer { stream, addr }
    }

    /// Handshaking with SOCKS5 client.
    /// Afterwards, authenticator should contain negotiated method.
    pub async fn process_socks5_handshake(&mut self, authenticator: &mut LurkAuthenticator) -> Result<()> {
        let request = self.stream.read_request::<HandshakeRequest>().await?;

        LurkSocks5RequestHandler::handle_handshake_request(self, request, authenticator).await
    }

    /// Handling SOCKS5 command which comes in relay request from client.
    pub async fn process_socks5_command(&mut self, server_address: SocketAddr) -> Result<()> {
        let request = self.stream.read_request::<RelayRequest>().await?;

        LurkSocks5RequestHandler::handle_relay_request(self, request, server_address).await
    }
}

impl<S> Display for LurkPeer<S>
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
    use crate::{common::LurkAuthMethod, io::stream::MockLurkStreamWrapper, proto::socks5::response::HandshakeResponse};
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

        let peer_methods = [LurkAuthMethod::None, LurkAuthMethod::GssAPI];
        let agreed_method = LurkAuthMethod::None;

        stream
            .expect_read_request()
            .once()
            .returning(move || Ok(HandshakeRequest::new(HashSet::from(peer_methods))));

        stream
            .expect_write_response()
            .once()
            .with(predicate::eq(HandshakeResponse::builder().with_auth_method(agreed_method).build()))
            .returning(|_| Ok(()));

        let mut peer = LurkPeer::new(stream, addr);
        let mut authenticator = LurkAuthenticator::new(false);

        peer.process_socks5_handshake(&mut authenticator).await.unwrap();
        assert_eq!(agreed_method, authenticator.current_method().unwrap());
    }
}
