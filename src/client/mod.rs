use self::handlers::LurkRequestHandler;
use crate::{
    auth::LurkAuthenticator,
    io::{stream::LurkStreamWrapper, LurkRequestRead, LurkResponseWrite},
    proto::socks5::{
        request::{HandshakeRequest, RelayRequest},
        response::RelayResponse,
    },
};
use anyhow::Result;
use log::debug;
use std::{
    fmt::Display,
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

mod handlers;

pub type LurkTcpClient = LurkClient<LurkStreamWrapper<TcpStream>>;

pub struct LurkClient<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin,
{
    addr: SocketAddr,
    stream: S,
}

impl<S> LurkClient<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin + DerefMut,
    <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S, addr: SocketAddr) -> LurkClient<S> {
        LurkClient { stream, addr }
    }

    /// Handshaking with SOCKS5 client.
    /// Afterwards, authenticator should contain negotiated method.
    pub async fn process_socks5_handshake(&mut self, authenticator: &mut LurkAuthenticator) -> Result<()> {
        let request = self.stream.read_request::<HandshakeRequest>().await?;

        LurkRequestHandler::handle_socks5_handshake_request(self, request, authenticator).await
    }

    /// Handling SOCKS5 command which comes in relay request from client.
    pub async fn process_socks5_command(&mut self, server_address: SocketAddr) -> Result<()> {
        let request = self.stream.read_request::<RelayRequest>().await?;

        if let Err(err) = LurkRequestHandler::handle_socks5_relay_request(self, request, server_address).await {
            let error_string = err.to_string();
            let response = RelayResponse::builder().with_err(err).with_bound_address(server_address).build();

            debug!("Error: '{}'. Response: '{:?}' to {}", error_string, response, self);
            self.stream.write_response(response).await?;
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
    use crate::{
        io::stream::MockLurkStreamWrapper,
        proto::socks5::{response::HandshakeResponse, AuthMethod},
    };
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
            .with(predicate::eq(HandshakeResponse::builder().with_auth_method(agreed_method).build()))
            .returning(|_| Ok(()));

        let mut client = LurkClient::new(stream, addr);
        let mut authenticator = LurkAuthenticator::new(false);

        client.process_socks5_handshake(&mut authenticator).await.unwrap();
        assert_eq!(agreed_method, authenticator.current_method().unwrap());
    }
}
