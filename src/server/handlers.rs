use crate::{
    auth::LurkAuthenticator,
    common::{error::LurkError, logging},
    io::{tunnel::LurkTunnel, LurkRequestRead, LurkResponseWrite},
    net::{
        tcp::{
            connection::{LurkTcpConnection, LurkTcpConnectionLabel},
            establish_tcp_connection_with_opts, TcpConnectionOptions,
        },
        Address,
    },
    proto::socks5::{
        request::{HandshakeRequest, RelayRequest},
        response::{HandshakeResponse, RelayResponse},
        Command,
    },
};
use anyhow::{bail, Result};
use human_bytes::human_bytes;
use log::{debug, error, info};
use socket2::TcpKeepalive;
use std::time::Duration;

pub struct LurkSocks5Handler {
    conn: LurkTcpConnection,
}

impl LurkSocks5Handler {
    pub fn new(conn: LurkTcpConnection) -> LurkSocks5Handler {
        LurkSocks5Handler { conn }
    }

    pub async fn handle(&mut self) -> Result<()> {
        debug_assert_eq!(LurkTcpConnectionLabel::SOCKS5, self.conn.label(), "expected SOCKS5 label");
        // Complete handshake process and authenticate the client on success.
        self.process_handshake().await?;
        // Proceed with SOCKS5 relay handling.
        // This will receive and process relay request, handle SOCKS5 command
        // and establish the tunnel "client <-- lurk proxy --> target".
        self.process_relay_request().await
    }

    /// Handshaking with SOCKS5 client.
    /// Afterwards, authenticator should contain negotiated method.
    async fn process_handshake(&mut self) -> Result<()> {
        let request = self.conn.stream_mut().read_request::<HandshakeRequest>().await?;

        // Authenticator will select method among all stored in request
        // and authenticate the connection on success.
        let mut authenticator = LurkAuthenticator::new();
        // Prepare builder for the response on handshake request.
        let mut response_builder = HandshakeResponse::builder();

        match authenticator.select_auth_method(request.auth_methods()) {
            Some(method) => {
                debug!("Selected authentication method {:?} for {}", method, self.conn.peer_addr());
                // Respond to the client with selected method.
                response_builder.with_auth_method(method);
                self.conn.stream_mut().write_response(response_builder.build()).await?;
                // Authenticate the client by using selected method.
                // Note: Currently, only None method (disabled auth) is supported,
                // so just a sanity check here.
                authenticator.authenticate_connection(&self.conn)
            }
            None => {
                debug!("No acceptable methods identified for {}", self.conn.peer_addr());
                response_builder.with_no_acceptable_method();
                self.conn.stream_mut().write_response(response_builder.build()).await
            }
        }
    }

    /// Handling SOCKS5 command which comes in relay request from client.
    async fn process_relay_request(&mut self) -> Result<()> {
        let request = self.conn.stream_mut().read_request::<RelayRequest>().await?;

        // Handle SOCKS5 command that encapsulated in relay request data.
        if let Err(err) = self.process_relay_request_impl(&request).await {
            let error_string = err.to_string();
            let response = RelayResponse::builder()
                .with_err(err)
                .with_bound_address(self.conn.local_addr())
                .build();

            logging::log_request_handling_error!(self.conn, error_string, request, response);
            self.conn.stream_mut().write_response(response).await?
        }

        Ok(())
    }

    async fn process_relay_request_impl(&mut self, request: &RelayRequest) -> Result<()> {
        // Handle SOCKS5 command that encapsulated in relay request data.
        match request.command() {
            Command::TCPConnect => self.process_socks5_connect(request.endpoint_address()).await,
            cmd => bail!(LurkError::UnsupportedSocksCommand(cmd)),
        }
    }

    async fn process_socks5_connect(&mut self, endpoint_address: &Address) -> Result<()> {
        let (conn_peer_addr, conn_bound_addr) = (self.conn.peer_addr(), self.conn.local_addr());
        debug!("Handling SOCKS5 CONNECT from {}", conn_peer_addr);

        // Create TCP options.
        let mut tcp_opts = TcpConnectionOptions::new();
        tcp_opts.set_keepalive(
            TcpKeepalive::new()
                .with_time(Duration::from_secs(300))    // 5 min
                .with_interval(Duration::from_secs(60)) // 1 min
                .with_retries(5),
        );

        // Establish TCP connection with the target endpoint.
        let mut r2l = establish_tcp_connection_with_opts(endpoint_address, &tcp_opts).await?;

        // Respond to relay request with success.
        let response = RelayResponse::builder()
            .with_success()
            .with_bound_address(self.conn.local_addr())
            .build();
        self.conn.stream_mut().write_response(response).await?;

        // Acquire mutable reference to inner object of stream wrapper.
        let mut l2r = &mut **self.conn.stream_mut();

        // Create proxy tunnel which operates with the following TCP streams:
        // - L2R: client   <--> proxy
        // - R2L: endpoint <--> proxy
        let mut tunnel = LurkTunnel::new(&mut l2r, &mut r2l);

        logging::log_tunnel_created!(conn_peer_addr, conn_bound_addr, endpoint_address);

        // Start data relaying
        match tunnel.run().await {
            Ok((l2r, r2l)) => {
                logging::log_tunnel_closed!(conn_peer_addr, conn_bound_addr, endpoint_address, l2r, r2l);
            }
            Err(err) => {
                logging::log_tunnel_closed_with_error!(conn_peer_addr, conn_bound_addr, endpoint_address, err);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::{common::LurkAuthMethod, io::stream::MockLurkStreamWrapper, proto::socks5::response::HandshakeResponse};
    // use mockall::predicate;
    // use std::{
    //     collections::HashSet,
    //     net::{IpAddr, Ipv4Addr},
    // };
    // use tokio_test::io::Mock;

    // #[tokio::test]
    // async fn socks5_handshake() {
    //     let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    //     let mut stream = MockLurkStreamWrapper::<Mock>::new();

    //     let peer_methods = [LurkAuthMethod::None, LurkAuthMethod::GssAPI];
    //     let agreed_method = LurkAuthMethod::None;

    //     stream
    //         .expect_read_request()
    //         .once()
    //         .returning(move || Ok(HandshakeRequest::new(HashSet::from(peer_methods))));

    //     stream
    //         .expect_write_response()
    //         .once()
    //         .with(predicate::eq(HandshakeResponse::builder().with_auth_method(agreed_method).build()))
    //         .returning(|_| Ok(()));

    //     let conn = LurkTcpConnection::new(stream, addr);
    //     let mut socks5_handler = LurkSocks5Handler::new(peer, "127.0.0.1:666".parse().unwrap());

    //     socks5_handler.process_handshake().await.unwrap();
    // }
}
