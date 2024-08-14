use crate::{
    auth::LurkAuthenticator,
    common::{error::LurkError, logging},
    io::{tunnel::LurkTunnel, LurkRequest, LurkResponse},
    net::tcp::{
        self,
        connection::{LurkTcpConnection, LurkTcpConnectionHandler, LurkTcpConnectionLabel},
    },
    proto::socks5::{
        request::{HandshakeRequest, RelayRequest},
        response::{HandshakeResponse, RelayResponse},
        Command,
    },
};
use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use human_bytes::human_bytes;
use log::{debug, error, info};

pub struct LurkSocks5Handler {}

impl LurkSocks5Handler {
    /// Handshaking with SOCKS5 client.
    /// Afterwards, authenticator should contain negotiated method.
    async fn process_handshake(conn: &mut LurkTcpConnection) -> Result<()> {
        let request = HandshakeRequest::read_from(conn.stream_mut()).await?;

        // Authenticator will select method among all stored in request
        // and authenticate the connection on success.
        let mut authenticator = LurkAuthenticator::new();
        // Prepare builder for the response on handshake request.
        let mut response_builder = HandshakeResponse::builder();

        match authenticator.select_auth_method(request.auth_methods()) {
            Some(method) => {
                debug!("Selected authentication method {:?} for {}", method, conn.peer_addr());
                // Respond to the client with selected method.
                response_builder.with_auth_method(method);
                response_builder.build().write_to(conn.stream_mut()).await?;
                // Authenticate the client by using selected method.
                // Note: Currently, only None method (disabled auth) is supported,
                // so just a sanity check here.
                authenticator.authenticate_connection(conn)
            }
            None => {
                debug!("No acceptable methods identified for {}", conn.peer_addr());
                response_builder.with_no_acceptable_method();
                response_builder.build().write_to(conn.stream_mut()).await?;
                bail!(LurkError::NoAcceptableAuthenticationMethod)
            }
        }
    }

    /// Handling SOCKS5 command which comes in relay request from client.
    async fn process_relay_request(conn: &mut LurkTcpConnection) -> Result<()> {
        let conn_peer_addr = conn.peer_addr();
        let conn_bound_addr = conn.local_addr();
        let inbound_stream = conn.stream_mut();
        let request = RelayRequest::read_from(inbound_stream).await?;
        let command = request.command();
        let address = request.endpoint_address();

        // Bail out and notify client if command isn't supported
        if command != Command::TCPConnect {
            return LurkSocks5Handler::on_relay_request_handling_error(
                anyhow!(LurkError::UnsupportedSocksCommand(command)),
                &request,
                conn,
            )
            .await;
        }

        info!("SOCKS5 CONNECT from peer {} to {}", conn_peer_addr, address);

        // Create TCP stream with the endpoint
        let mut outbound_stream = match tcp::establish_tcp_connection(address.to_socket_addr().await?).await {
            Ok(outbound_stream) => {
                // On success, respond to relay request with success
                let response = RelayResponse::builder().with_success().with_bound_address(conn_bound_addr).build();
                response.write_to(inbound_stream).await?;

                outbound_stream
            }
            Err(err) => return LurkSocks5Handler::on_relay_request_handling_error(err, &request, conn).await,
        };

        // Create proxy tunnel which operates with the following TCP streams:
        // - L2R: client   <--> proxy
        // - R2L: endpoint <--> proxy
        let mut tunnel = LurkTunnel::new(inbound_stream, &mut outbound_stream);

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

    async fn on_relay_request_handling_error(err: anyhow::Error, request: &RelayRequest, conn: &mut LurkTcpConnection) -> Result<()> {
        let err_msg = err.to_string();
        let response = RelayResponse::builder().with_err(err).with_bound_address(conn.local_addr()).build();

        logging::log_request_handling_error!(conn, err_msg, request, response);
        response.write_to(conn.stream_mut()).await
    }
}

#[async_trait]
impl LurkTcpConnectionHandler for LurkSocks5Handler {
    async fn handle(&mut self, mut conn: LurkTcpConnection) -> Result<()> {
        debug_assert_eq!(LurkTcpConnectionLabel::Socks5, conn.label(), "expected SOCKS5 label");
        // Complete handshake process and authenticate the client on success.
        LurkSocks5Handler::process_handshake(&mut conn).await?;
        // Proceed with SOCKS5 relay handling.
        // This will receive and process relay request, handle SOCKS5 command
        // and establish the tunnel "client <-- lurk proxy --> target".
        LurkSocks5Handler::process_relay_request(&mut conn).await
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
    async fn handshake_with_auth_method() {
        let mut listener = LurkTcpListener::bind(TEST_BIND_IPV4).await.expect("Expect binded listener");

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

        let mut conn = listener.accept().await.expect("Expect created connection");
        assert_eq!(LurkTcpConnectionLabel::Socks5, conn.label());
        assert_ok!(LurkSocks5Handler::process_handshake(&mut conn).await);

        assert_ok!(client_handle.into_future().await);
    }

    #[tokio::test]
    async fn handshake_with_non_accepatable_method() {
        let mut listener = LurkTcpListener::bind(TEST_BIND_IPV4).await.expect("Expect binded listener");

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

        let mut conn = listener.accept().await.expect("Expect created connection");
        assert_eq!(LurkTcpConnectionLabel::Socks5, conn.label());
        assert_lurk_err!(
            LurkError::NoAcceptableAuthenticationMethod,
            LurkSocks5Handler::process_handshake(&mut conn).await.expect_err("Expect error")
        );

        assert_ok!(client_handle.into_future().await);
    }
}
