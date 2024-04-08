use super::LurkPeer;
use crate::{
    common::{
        error::{LurkError, Unsupported},
        net::Address,
    },
    io::{tunnel::LurkTunnel, LurkRequestRead, LurkResponseWrite},
    proto::socks5::{
        request::{HandshakeRequest, RelayRequest},
        response::{HandshakeResponse, RelayResponse},
        Command,
    },
    server::auth::LurkAuthenticator,
};
use anyhow::{bail, Error, Result};
use log::debug;
use std::{
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub struct LurkRequestHandler {}

impl LurkRequestHandler {
    pub async fn handle_socks5_handshake_request<S>(
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
        } else {
            response_builder.with_no_acceptable_method();
        }
        // Communicate selected authentication method to the client.
        peer.stream.write_response(response_builder.build()).await
    }

    pub async fn handle_socks5_relay_request<S>(peer: &mut LurkPeer<S>, request: RelayRequest, server_address: SocketAddr) -> Result<()>
    where
        S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
        <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
    {
        let mut command_handler = LurkCommandHandler::new(peer);

        // Handle SOCKS5 command that encapsulated in relay request data.
        let result = match request.command() {
            Command::Connect => {
                command_handler
                    .handle_socks5_connect(server_address, request.endpoint_address())
                    .await
            }
            cmd => bail!(LurkError::Unsupported(Unsupported::Socks5Command(cmd))),
        };

        // If error occured, handle it with respond to processing relay request.
        if let Err(err) = result {
            LurkRequestHandler::handle_error_with_response(peer, server_address, err).await?;
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

struct LurkCommandHandler<'a, S>
where
    S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
{
    peer: &'a mut LurkPeer<S>,
}

impl<'a, S> LurkCommandHandler<'a, S>
where
    S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
    <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(peer: &'a mut LurkPeer<S>) -> LurkCommandHandler<'a, S> {
        LurkCommandHandler { peer }
    }

    pub async fn handle_socks5_connect(&mut self, server_address: SocketAddr, endpoint_address: &Address) -> Result<()> {
        debug!("Handling SOCKS5 CONNECT from {}", self.peer);
        debug!(
            "Starting data relaying tunnel: client [{}] <---> lurk [{}] <---> destination [{}]",
            self.peer, server_address, endpoint_address
        );

        // Resolve endpoint address.
        let endpoint_address = endpoint_address.to_socket_addr().await?;

        // Establish TCP connection with the endpoint.
        debug!("Establishing TCP connection with the endpoint {} ... ", endpoint_address);
        let mut r2l = TcpStream::connect(endpoint_address).await.map_err(anyhow::Error::from)?;
        debug!("TCP connection has been established with the endpoint {}", endpoint_address);

        // Respond to relay request with success.
        let response = RelayResponse::builder().with_success().with_bound_address(server_address).build();
        self.peer.stream.write_response(response).await?;

        let mut l2r = &mut *self.peer.stream;

        // Create proxy tunnel which operates with the following TCP streams:
        // - L2R: client   <--> proxy
        // - R2L: endpoint <--> proxy
        let mut tunnel = LurkTunnel::new(&mut l2r, &mut r2l);

        // Start data relaying
        tunnel.run().await;

        Ok(())
    }
}
