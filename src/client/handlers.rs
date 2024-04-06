use super::LurkClient;
use crate::{
    auth::LurkAuthenticator,
    error::{LurkError, Unsupported},
    io::{tunnel::LurkTunnel, LurkRequestRead, LurkResponseWrite},
    proto::socks5::{
        request::{HandshakeRequest, RelayRequest},
        response::{HandshakeResponse, RelayResponse},
        Command,
    },
};
use anyhow::{bail, Result};
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
        client: &mut LurkClient<S>,
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
        }
        // Communicate selected authentication method to client.
        client.stream.write_response(response_builder.build()).await
    }

    pub async fn handle_socks5_relay_request<'a, S>(
        client: &mut LurkClient<S>,
        request: RelayRequest,
        server_address: SocketAddr,
    ) -> Result<()>
    where
        S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
        <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
    {
        let mut command_handler = LurkCommandHandler::new(client);
        let target_address = request.target_addr().to_socket_addr().await?;

        match request.command() {
            Command::Connect => command_handler.handle_socks5_connect(server_address, target_address).await,
            cmd => bail!(LurkError::Unsupported(Unsupported::Socks5Command(cmd))),
        }
    }
}

struct LurkCommandHandler<'a, S>
where
    S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
{
    client: &'a mut LurkClient<S>,
}

impl<'a, S> LurkCommandHandler<'a, S>
where
    S: LurkRequestRead + LurkResponseWrite + DerefMut + Unpin,
    <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(client: &'a mut LurkClient<S>) -> LurkCommandHandler<'a, S> {
        LurkCommandHandler { client }
    }

    pub async fn handle_socks5_connect(&mut self, server_address: SocketAddr, endpoint_address: SocketAddr) -> Result<()> {
        debug!("Handling SOCKS5 CONNECT from {}", self.client);
        debug!(
            "Starting data relaying tunnel: client [{}] <---> lurk [{}] <---> destination [{}]",
            self.client, server_address, endpoint_address
        );

        // Establish TCP connection with the endpoint.
        debug!("Establishing TCP connection with the endpoint {} ... ", endpoint_address);
        let mut r2l = TcpStream::connect(endpoint_address).await.map_err(anyhow::Error::from)?;
        debug!("TCP connection has been established with the endpoint {}", endpoint_address);

        // Respond to relay request with success.
        let response = RelayResponse::builder().with_success().with_bound_address(server_address).build();
        self.client.stream.write_response(response).await?;

        let mut l2r = &mut *self.client.stream;

        // Create proxy tunnel which operates with the following TCP streams:
        // - L2R: client   <--> proxy
        // - R2L: endpoint <--> proxy
        let mut tunnel = LurkTunnel::new(&mut l2r, &mut r2l);

        // Start data relaying
        tunnel.run().await
    }
}
