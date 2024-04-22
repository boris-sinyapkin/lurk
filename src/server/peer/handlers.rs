use super::{auth::LurkAuthenticator, LurkPeer, LurkPeerType};
use crate::{
    common::{
        error::LurkError,
        logging::{log_request_handling_error, log_tunnel_closed, log_tunnel_closed_with_error, log_tunnel_created},
        net::Address,
    },
    io::{tunnel::LurkTunnel, LurkRequestRead, LurkResponseWrite},
    proto::socks5::{
        request::{HandshakeRequest, RelayRequest},
        response::{HandshakeResponse, RelayResponse},
        Command,
    },
};
use anyhow::{bail, Result};
use human_bytes::human_bytes;
use log::{debug, error, info, trace};
use socket2::{SockRef, TcpKeepalive};
use std::{
    net::SocketAddr,
    ops::{Deref, DerefMut},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub struct LurkPeerHandler<S>
where
    S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
{
    peer: LurkPeer<S>,
    server_address: SocketAddr,
}

impl<S> LurkPeerHandler<S>
where
    S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
    <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(peer: LurkPeer<S>, server_address: SocketAddr) -> LurkPeerHandler<S> {
        LurkPeerHandler { peer, server_address }
    }

    pub async fn handle(&mut self) -> Result<()> {
        match self.peer.peer_type() {
            LurkPeerType::SOCKS5 => {
                // Complete handshake process and authenticate the client on success.
                self.process_socks5_handshake().await?;
                // Proceed with SOCKS5 relay handling.
                // This will receive and process relay request, handle SOCKS5 command
                // and establish the tunnel "client <-- lurk proxy --> target".
                self.process_socks5_relay_request().await
            }
        }
    }

    /// Handshaking with SOCKS5 client.
    /// Afterwards, authenticator should contain negotiated method.
    async fn process_socks5_handshake(&mut self) -> Result<()> {
        let request = self.peer.stream.read_request::<HandshakeRequest>().await?;

        if let Err(err) = self.process_socks5_handshake_impl(&request).await {
            log_request_handling_error!(self.peer, err, request, ());
        }

        Ok(())
    }

    /// Handling SOCKS5 command which comes in relay request from client.
    async fn process_socks5_relay_request(&mut self) -> Result<()> {
        let request = self.peer.stream.read_request::<RelayRequest>().await?;

        // Handle SOCKS5 command that encapsulated in relay request data.
        if let Err(err) = self.process_socks5_relay_request_impl(&request).await {
            let error_string = err.to_string();
            let response = RelayResponse::builder()
                .with_err(err)
                .with_bound_address(self.server_address)
                .build();

            log_request_handling_error!(self.peer, error_string, request, response);
            self.peer.stream.write_response(response).await?
        }

        Ok(())
    }

    async fn process_socks5_handshake_impl(&mut self, request: &HandshakeRequest) -> Result<()> {
        // Create authenticator.
        let mut authenticator = LurkAuthenticator::new();
        // Create response builder.
        let mut response_builder = HandshakeResponse::builder();

        // Select the authentication method.
        if let Some(method) = authenticator.select_auth_method(request.auth_methods()) {
            debug!("Selected authentication method {:?} for {}", method, self.peer);

            // Prepare and send the response with selected method.
            response_builder.with_auth_method(method);
            self.peer.stream.write_response(response_builder.build()).await?;

            // Authenticate the client.
            debug_assert!(authenticator.authenticate(&self.peer));
        } else {
            // Method hasn't been selected.
            debug!("No acceptable methods identified for for {}", self.peer);

            // Notify client and bail out.
            response_builder.with_no_acceptable_method();
            self.peer.stream.write_response(response_builder.build()).await?;

            bail!(LurkError::NoAcceptableAuthMethod)
        }

        Ok(())
    }

    async fn process_socks5_relay_request_impl(&mut self, request: &RelayRequest) -> Result<()> {
        // Handle SOCKS5 command that encapsulated in relay request data.
        match request.command() {
            Command::TCPConnect => self.process_socks5_connect(request.endpoint_address()).await,
            cmd => bail!(LurkError::UnsupportedSocksCommand(cmd)),
        }
    }

    async fn process_socks5_connect(&mut self, endpoint_address: &Address) -> Result<()>
    where
        S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
        <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("Handling SOCKS5 CONNECT from {}", self.peer);
        let peer_address = self.peer.to_string();

        // Resolve endpoint address.
        trace!("Endpoint address {} resolution: ... ", endpoint_address);
        let resolved_address = endpoint_address.to_socket_addr().await?;
        trace!(
            "Endpoint address {} resolution: SUCCESS with {}",
            endpoint_address,
            resolved_address
        );

        // Establish TCP connection with the endpoint.
        debug!("TCP connection establishment with the endpoint {}: ... ", endpoint_address);
        let mut r2l = TcpStream::connect(resolved_address).await.map_err(anyhow::Error::from)?;
        debug!("TCP connection establishment with the endpoint {}: SUCCESS", endpoint_address);

        // Configure keep-alive probe.
        let r2l_sock_ref = SockRef::from(&r2l);
        let keepalive = TcpKeepalive::new()
            .with_time(Duration::from_secs(300))    // 5 min
            .with_interval(Duration::from_secs(60)) // 1 min
            .with_retries(5);

        // Configure keep-alive probe on R2L TCP socket.
        r2l_sock_ref.set_tcp_keepalive(&keepalive)?;

        // Respond to relay request with success.
        let response = RelayResponse::builder()
            .with_success()
            .with_bound_address(self.server_address)
            .build();
        self.peer.stream.write_response(response).await?;

        let mut l2r = &mut *self.peer.stream;

        // Create proxy tunnel which operates with the following TCP streams:
        // - L2R: client   <--> proxy
        // - R2L: endpoint <--> proxy
        let mut tunnel = LurkTunnel::new(&mut l2r, &mut r2l);

        log_tunnel_created!(peer_address, self.server_address, endpoint_address);

        // Start data relaying
        match tunnel.run().await {
            Ok((l2r, r2l)) => {
                log_tunnel_closed!(peer_address, self.server_address, endpoint_address, l2r, r2l);
            }
            Err(err) => {
                log_tunnel_closed_with_error!(peer_address, self.server_address, endpoint_address, err);
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

        let peer = LurkPeer::new(stream, addr, LurkPeerType::SOCKS5);
        let mut socks5_handler = LurkPeerHandler::new(peer, "127.0.0.1:666".parse().unwrap());

        socks5_handler.process_socks5_handshake().await.unwrap();
    }
}
