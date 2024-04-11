use super::{auth::LurkAuthenticator, LurkPeer};
use crate::{
    common::{
        error::{unsupported, LurkError, Unsupported},
        net::Address,
    },
    io::{
        tunnel::{log_tunnel_closed, log_tunnel_closed_with_error, log_tunnel_created, LurkTunnel},
        LurkRequestRead, LurkResponseWrite,
    },
    proto::socks5::{
        request::{HandshakeRequest, RelayRequest},
        response::{HandshakeResponse, RelayResponse},
        Command,
    },
};
use anyhow::{bail, Error, Result};
use human_bytes::human_bytes;
use log::{debug, error, info};
use std::{
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub struct LurkSocks5PeerHandler<'a, S>
where
    S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
{
    peer: &'a mut LurkPeer<S>,
    authenticator: LurkAuthenticator,
    server_address: SocketAddr,
}

impl<'a, S> LurkSocks5PeerHandler<'a, S>
where
    S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
    <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(peer: &'a mut LurkPeer<S>, server_address: SocketAddr) -> LurkSocks5PeerHandler<'a, S> {
        LurkSocks5PeerHandler {
            peer,
            server_address,
            authenticator: LurkAuthenticator::new(),
        }
    }

    pub async fn handle(&mut self) -> Result<()> {
        // Complete handshake process and negotiate the authentication method.
        self.process_handshake().await?;

        // Authenticate client with selected method.
        self.authenticator.authenticate(self.peer);

        // Proceed with SOCKS5 relay handling.
        // This will receive and process relay request, handle SOCKS5 command
        // and establish the tunnel "client <-- lurk proxy --> target".
        self.process_command().await
    }

    /// Handshaking with SOCKS5 client.
    /// Afterwards, authenticator should contain negotiated method.
    async fn process_handshake(&mut self) -> Result<()> {
        let request = self.peer.stream.read_request::<HandshakeRequest>().await?;

        LurkSocks5RequestHandler::handle_handshake_request(self.peer, request, &mut self.authenticator).await
    }

    /// Handling SOCKS5 command which comes in relay request from client.
    async fn process_command(&mut self) -> Result<()> {
        let request = self.peer.stream.read_request::<RelayRequest>().await?;

        LurkSocks5RequestHandler::handle_relay_request(self.peer, request, self.server_address).await
    }
}

struct LurkSocks5RequestHandler {}

impl LurkSocks5RequestHandler {
    pub async fn handle_handshake_request<S>(
        peer: &mut LurkPeer<S>,
        request: HandshakeRequest,
        authenticator: &mut LurkAuthenticator,
    ) -> Result<()>
    where
        S: LurkRequestRead + LurkResponseWrite + Unpin,
    {
        // Pick authentication method.
        authenticator.select_auth_method(request.auth_methods());

        // Prepare response.
        let mut response_builder = HandshakeResponse::builder();
        if let Some(method) = authenticator.current_method() {
            response_builder.with_auth_method(method);
            info!("Selected authentication method {:?} for {}", method, peer);
        } else {
            response_builder.with_no_acceptable_method();
            info!("No acceptable methods identified for for {}", peer);
        }

        // Communicate selected authentication method to the client.
        peer.stream.write_response(response_builder.build()).await
    }

    pub async fn handle_relay_request<S>(peer: &mut LurkPeer<S>, request: RelayRequest, server_address: SocketAddr) -> Result<()>
    where
        S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
        <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
    {
        // Handle SOCKS5 command that encapsulated in relay request data.
        let result = match request.command() {
            Command::Connect => LurkSocks5CommandHandler::handle_connect(peer, server_address, request.endpoint_address()).await,
            cmd => unsupported!(Unsupported::Socks5Command(cmd)),
        };

        // If error occured, handle it with respond to processing relay request.
        if let Err(err) = result {
            LurkSocks5RequestHandler::handle_error_with_response(peer, server_address, err).await?;
        }

        Ok(())
    }

    async fn handle_error_with_response<S>(peer: &mut LurkPeer<S>, server_address: SocketAddr, err: Error) -> Result<()>
    where
        S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
    {
        let error_string = err.to_string();
        let response = RelayResponse::builder().with_err(err).with_bound_address(server_address).build();

        debug!("Error: '{}'. Response: '{:?}' to {}", error_string, response, peer);
        peer.stream.write_response(response).await
    }
}

struct LurkSocks5CommandHandler {}

impl LurkSocks5CommandHandler {
    pub async fn handle_connect<S>(peer: &mut LurkPeer<S>, server_address: SocketAddr, endpoint_address: &Address) -> Result<()>
    where
        S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
        <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
    {
        info!("Handling SOCKS5 CONNECT from {}", peer);
        let peer_address = peer.to_string();

        // Resolve endpoint address.
        debug!("Resolving endpoint address {} ... ", endpoint_address);
        let resolved_address = endpoint_address.to_socket_addr().await?;
        debug!("Resolved endpoint address {} to {}", endpoint_address, resolved_address);

        // Establish TCP connection with the endpoint.
        debug!("Establishing TCP connection with the endpoint {} ... ", endpoint_address);
        let mut r2l = TcpStream::connect(resolved_address).await.map_err(anyhow::Error::from)?;
        debug!("TCP connection has been established with the endpoint {}", endpoint_address);

        // Respond to relay request with success.
        let response = RelayResponse::builder().with_success().with_bound_address(server_address).build();
        peer.stream.write_response(response).await?;

        let mut l2r = &mut *peer.stream;

        // Create proxy tunnel which operates with the following TCP streams:
        // - L2R: client   <--> proxy
        // - R2L: endpoint <--> proxy
        let mut tunnel = LurkTunnel::new(&mut l2r, &mut r2l);

        log_tunnel_created!(peer_address, server_address, endpoint_address);

        // Start data relaying
        match tunnel.run().await {
            Ok((l2r, r2l)) => {
                log_tunnel_closed!(peer_address, server_address, endpoint_address, l2r, r2l);
            }
            Err(err) => {
                log_tunnel_closed_with_error!(peer_address, server_address, endpoint_address, err);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        common::LurkAuthMethod, io::stream::MockLurkStreamWrapper, proto::socks5::response::HandshakeResponse, server::peer::LurkPeerType,
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

        let mut peer = LurkPeer::new(stream, addr, LurkPeerType::Socks5Peer);
        let mut socks5_handler = LurkSocks5PeerHandler::new(&mut peer, "127.0.0.1:666".parse().unwrap());

        socks5_handler.process_handshake().await.unwrap();
        assert_eq!(agreed_method, socks5_handler.authenticator.current_method().unwrap());
    }
}
