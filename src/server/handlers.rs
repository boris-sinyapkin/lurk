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
use tokio::net::TcpStream;

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
                self.conn.stream_mut().write_response(response_builder.build()).await?;
                bail!(LurkError::NoAcceptableAuthenticationMethod)
            }
        }
    }

    /// Handling SOCKS5 command which comes in relay request from client.
    async fn process_relay_request(&mut self) -> Result<()> {
        let request = self.conn.stream_mut().read_request::<RelayRequest>().await?;
        let command = request.command();
        let address = request.endpoint_address();

        // Bail out and notify client if command isn't supported
        if command != Command::TCPConnect {
            let err = anyhow::anyhow!(LurkError::UnsupportedSocksCommand(command));
            return self.on_relay_request_handling_error(err, &request).await;
        }

        let (conn_peer_addr, conn_bound_addr) = (self.conn.peer_addr(), self.conn.local_addr());
        debug!("Handling SOCKS5 CONNECT from {}", conn_peer_addr);

        // Create TCP stream with the endpoint
        let mut r2l = match self.establish_tcp_connection(address).await {
            Ok(stream) => {
                // On success, respond to relay request with success
                let response = RelayResponse::builder().with_success().with_bound_address(conn_bound_addr).build();
                self.conn.stream_mut().write_response(response).await?;

                stream
            }
            Err(err) => return self.on_relay_request_handling_error(err, &request).await,
        };

        // Acquire mutable reference to inner object of stream wrapper.
        let mut l2r = &mut **self.conn.stream_mut();

        // Create proxy tunnel which operates with the following TCP streams:
        // - L2R: client   <--> proxy
        // - R2L: endpoint <--> proxy
        let mut tunnel = LurkTunnel::new(&mut l2r, &mut r2l);

        logging::log_tunnel_created!(conn_peer_addr, conn_bound_addr, address);

        // Start data relaying
        match tunnel.run().await {
            Ok((l2r, r2l)) => {
                logging::log_tunnel_closed!(conn_peer_addr, conn_bound_addr, address, l2r, r2l);
            }
            Err(err) => {
                logging::log_tunnel_closed_with_error!(conn_peer_addr, conn_bound_addr, address, err);
            }
        }

        Ok(())
    }

    async fn on_relay_request_handling_error(&mut self, err: anyhow::Error, request: &RelayRequest) -> Result<()> {
        let err_msg = err.to_string();
        let response = RelayResponse::builder()
            .with_err(err)
            .with_bound_address(self.conn.local_addr())
            .build();

        logging::log_request_handling_error!(self.conn, err_msg, request, response);
        self.conn.stream_mut().write_response(response).await
    }

    async fn establish_tcp_connection(&mut self, endpoint_address: &Address) -> Result<TcpStream> {
        // Create TCP options.
        let mut tcp_opts = TcpConnectionOptions::new();
        tcp_opts.set_keepalive(
            TcpKeepalive::new()
                .with_time(Duration::from_secs(300))    // 5 min
                .with_interval(Duration::from_secs(60)) // 1 min
                .with_retries(5),
        );

        // Establish TCP connection with the target endpoint.
        establish_tcp_connection_with_opts(endpoint_address, &tcp_opts).await
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{auth::LurkAuthMethod, common::assertions::assert_lurk_err, net::tcp::listener::LurkTcpListener};
    use futures::TryFutureExt;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use tokio::net::TcpStream;
    use tokio_test::assert_ok;

    // :0 tells the OS to pick an open port.
    const TEST_BIND_IPV4: &str = "127.0.0.1:0";

    #[tokio::test]
    async fn socks5_handshake_with_auth_method() {
        let mut listener = LurkTcpListener::bind(TEST_BIND_IPV4)
            .await
            .expect("Expect binded listener");

        let listener_addr = listener.local_addr();
        let client_handle = tokio::spawn(async move {
            TcpStream::connect(listener_addr)
                .and_then(|mut s| async move {
                    // Send handshake request with auth methods.
                    HandshakeRequest::new(HashSet::from([
                        LurkAuthMethod::None,
                        LurkAuthMethod::GssAPI,
                        LurkAuthMethod::Password,
                    ]))
                    .write_to(&mut s)
                    .await;

                    // Read and verify handshake response.
                    let actual = HandshakeResponse::read_from(&mut s).await;
                    let reference = HandshakeResponse::builder().with_auth_method(LurkAuthMethod::None).build();

                    assert_eq!(reference, actual);
                    Ok(())
                })
                .await
                .unwrap()
        });

        tokio::task::yield_now().await;

        let conn = listener.accept().await.expect("Expect created connection");
        assert_eq!(LurkTcpConnectionLabel::SOCKS5, conn.label());

        let mut handler = LurkSocks5Handler::new(conn);
        assert_ok!(handler.process_handshake().await);

        assert_ok!(client_handle.into_future().await);
    }

    #[tokio::test]
    async fn socks5_handshake_with_non_accepatable_method() {
        let mut listener = LurkTcpListener::bind(TEST_BIND_IPV4)
            .await
            .expect("Expect binded listener");

        let listener_addr = listener.local_addr();
        let client_handle = tokio::spawn(async move {
            TcpStream::connect(listener_addr)
                .and_then(|mut s| async move {
                    // Send handshake request with auth methods.
                    HandshakeRequest::new(HashSet::from([LurkAuthMethod::GssAPI, LurkAuthMethod::Password]))
                        .write_to(&mut s)
                        .await;

                    // Read and verify handshake response.
                    let actual = HandshakeResponse::read_from(&mut s).await;
                    let reference = HandshakeResponse::builder().with_no_acceptable_method().build();

                    assert_eq!(reference, actual);
                    Ok(())
                })
                .await
                .unwrap()
        });

        tokio::task::yield_now().await;

        let conn = listener.accept().await.expect("Expect created connection");
        assert_eq!(LurkTcpConnectionLabel::SOCKS5, conn.label());

        let mut handler = LurkSocks5Handler::new(conn);
        assert_lurk_err!(
            LurkError::NoAcceptableAuthenticationMethod,
            handler.process_handshake().await.expect_err("Expect error")
        );

        assert_ok!(client_handle.into_future().await);
    }
}
