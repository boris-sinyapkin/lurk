use crate::{
    auth::LurkAuthenticator,
    common::{error::LurkError, logging},
    io::{tunnel::LurkTunnel, LurkRequest, LurkResponse},
    net::{
        tcp::{
            connection::{LurkTcpConnection, LurkTcpConnectionHandler, LurkTcpConnectionLabel},
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
use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use human_bytes::human_bytes;
use hyper::{
    client,
    server::{self},
    service::service_fn,
    Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use log::{debug, error, info, log_enabled, trace};
use socket2::TcpKeepalive;
use std::time::Duration;
use tokio::net::TcpStream;

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
        let peer_addr = conn.peer_addr();
        let local_addr = conn.local_addr();
        let inbound_stream = conn.stream_mut();
        let request = RelayRequest::read_from(inbound_stream).await?;
        let command = request.command();
        let address = request.endpoint_address();

        // Bail out and notify client if command isn't supported
        if command != Command::TCPConnect {
            let err = anyhow::anyhow!(LurkError::UnsupportedSocksCommand(command));
            return LurkSocks5Handler::on_relay_request_handling_error(err, &request, conn).await;
        }

        let (conn_peer_addr, conn_bound_addr) = (peer_addr, local_addr);
        debug!("Handling SOCKS5 CONNECT from {}", conn_peer_addr);

        // Create TCP stream with the endpoint
        let mut outbound_stream = match LurkSocks5Handler::establish_tcp_connection(address).await {
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

    async fn establish_tcp_connection(endpoint_address: &Address) -> Result<TcpStream> {
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

pub struct LurkHttpHandler {}

impl LurkHttpHandler {
    async fn serve_request(request: Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>> {
        // Dump full request data if trace is enabled
        if log_enabled!(log::Level::Trace) {
            trace!("{:?}", request);
        } else {
            info!("{:?} {} '{}'", request.version(), request.method(), request.uri());
        }

        if request.method() == Method::CONNECT {
            let addr_str = match request.uri().authority() {
                Some(str) => str.to_string(),
                None => {
                    let err_msg = format!("CONNECT host is not socket addr: {:?}", request.uri());
                    let mut response = Response::new(Self::full(err_msg));
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(response);
                }
            };

            tokio::spawn(async move {
                // Upgrage HTTP connection.
                let upgraded = match hyper::upgrade::on(request).await {
                    Ok(upgraded) => upgraded,
                    Err(err) => {
                        error!("HTTP upgrade error: {}", err);
                        return;
                    }
                };

                // On successful upgrade, establish remote TCP connection
                // and start data relaying.
                match TcpStream::connect(addr_str).await {
                    Ok(mut outbound) => {
                        let mut inbdound = TokioIo::new(upgraded);
                        let mut tunnel = LurkTunnel::new(&mut inbdound, &mut outbound);

                        // Start tunnel.
                        if let Err(err) = tunnel.run().await {
                            error!("Error occurred while tunnel was running: {}", err);
                        }
                    }
                    Err(err) => {
                        error!("Failed to establish outbound TCP connection: {}", err);
                    }
                }
            });

            Ok(Response::new(Self::empty()))
        } else {
            let host = request.uri().host().ok_or(anyhow!("HTTP request has no host"))?;
            let port = request.uri().port_u16().unwrap_or(80);

            let stream = TcpStream::connect((host, port)).await?;
            let io = TokioIo::new(stream);

            let (mut sender, conn) = client::conn::http1::Builder::new().handshake(io).await?;

            // Spawn a task to poll the connection and drive the HTTP state.
            tokio::spawn(async move {
                if let Err(err) = conn.await {
                    error!("Connection failed: {:?}", err);
                }
            });

            // Send request on associated connection.
            let response = sender.send_request(request).await?;

            Ok(response.map(|r| r.boxed()))
        }
    }

    //
    // Routines taken from example of proxy implementation based on hyper:
    // https://github.com/hyperium/hyper/blob/master/examples/http_proxy.rs
    //
    fn empty() -> BoxBody<Bytes, hyper::Error> {
        Empty::<Bytes>::new().map_err(|never| match never {}).boxed()
    }

    fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
        Full::new(chunk.into()).map_err(|never| match never {}).boxed()
    }
}

#[async_trait]
impl LurkTcpConnectionHandler for LurkHttpHandler {
    async fn handle(&mut self, conn: LurkTcpConnection) -> Result<()> {
        let io = TokioIo::from(conn);
        server::conn::http1::Builder::new()
            .serve_connection(io, service_fn(LurkHttpHandler::serve_request))
            .with_upgrades()
            .await
            .map_err(anyhow::Error::from)
    }
}

pub fn create_tcp_connection_handler(label: &LurkTcpConnectionLabel) -> Result<Box<dyn LurkTcpConnectionHandler>> {
    match label {
        LurkTcpConnectionLabel::Http | LurkTcpConnectionLabel::HttpSecure => Ok(Box::new(LurkHttpHandler {})),
        LurkTcpConnectionLabel::Socks5 => Ok(Box::new(LurkSocks5Handler {})),
        LurkTcpConnectionLabel::Unknown(_) => bail!("Unknown TCP connection"),
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
    async fn socks5_handshake_with_non_accepatable_method() {
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
