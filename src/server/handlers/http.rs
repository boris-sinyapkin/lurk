use crate::{
    io::tunnel::LurkTunnel,
    net::tcp::{
        self,
        connection::{LurkTcpConnection, LurkTcpConnectionHandler},
    },
};
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{
    client,
    server::{self},
    service::service_fn,
    Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use log::{error, info, log_enabled, trace};
use tokio::net::TcpStream;

pub struct LurkHttpHandler {}

impl LurkHttpHandler {
    async fn serve_request(mut request: Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>> {
        // Dump full request data if trace is enabled
        if log_enabled!(log::Level::Trace) {
            trace!("{:?}", request);
        } else {
            info!("{:?} {} '{}'", request.version(), request.method(), request.uri());
        }

        // Get remote host address from the request.
        let remote_addr = match utils::get_host_addr(&mut request) {
            Some(addr) => addr.to_socket_addr().await?,
            None => {
                error!("Failed to get remote host address");
                return Ok(Self::bad_request());
            }
        };

        if request.method() == Method::CONNECT {
            let mut outbound = match tcp::establish_tcp_connection(remote_addr).await {
                Ok(outbound) => outbound,
                Err(err) => {
                    error!("Failed to establish outbound TCP connection: {}", err);
                    return Ok(Self::server_error());
                }
            };

            tokio::spawn(async move {
                // Upgrage HTTP connection.
                let mut inbound = match hyper::upgrade::on(request).await {
                    Ok(upgraded) => TokioIo::new(upgraded),
                    Err(err) => {
                        error!("HTTP upgrade error: {}", err);
                        return;
                    }
                };

                let mut tunnel = LurkTunnel::new(&mut inbound, &mut outbound);

                // Start tunnel.
                if let Err(err) = tunnel.run().await {
                    error!("Error occurred while tunnel was running: {}", err);
                }
            });

            Ok(Self::ok())
        } else {
            let stream = TcpStream::connect(remote_addr).await?;
            let io = TokioIo::new(stream);

            let (mut sender, conn) = client::conn::http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .handshake(io)
                .await?;

            // Spawn a task to poll the connection and drive the HTTP state.
            tokio::spawn(async move {
                if let Err(err) = conn.await {
                    error!("Connection failed: {:?}", err);
                }
            });

            // Send request on associated connection.
            let response = sender.send_request(request).await?;
            trace!("{:?}", response);

            Ok(response.map(|r| r.boxed()))
        }
    }

    //
    // Routines taken from example of proxy implementation based on hyper:
    // https://github.com/hyperium/hyper/blob/master/examples/http_proxy.rs
    //
    fn empty_body() -> BoxBody<Bytes, hyper::Error> {
        Empty::<Bytes>::new().map_err(|never| match never {}).boxed()
    }

    #[allow(dead_code)]
    fn full_body<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
        Full::new(chunk.into()).map_err(|never| match never {}).boxed()
    }

    ///
    /// HTTP responses.
    ///
    fn bad_request() -> Response<BoxBody<Bytes, hyper::Error>> {
        Self::response(Self::empty_body(), StatusCode::BAD_REQUEST)
    }

    fn server_error() -> Response<BoxBody<Bytes, hyper::Error>> {
        Self::response(Self::empty_body(), StatusCode::INTERNAL_SERVER_ERROR)
    }

    fn ok() -> Response<BoxBody<Bytes, hyper::Error>> {
        Self::response(Self::empty_body(), StatusCode::OK)
    }

    fn response<T>(body: T, status: StatusCode) -> Response<T> {
        Response::builder().status(status).body(body).expect("HTTP response was not built")
    }
}

#[async_trait]
impl LurkTcpConnectionHandler for LurkHttpHandler {
    async fn handle(&mut self, conn: LurkTcpConnection) -> Result<()> {
        server::conn::http1::Builder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .serve_connection(TokioIo::from(conn), service_fn(LurkHttpHandler::serve_request))
            .with_upgrades()
            .await
            .map_err(anyhow::Error::from)
    }
}

mod utils {
    use crate::net::{ipv4_socket_address, ipv6_socket_address, Address};
    use anyhow::Result;
    use hyper::{
        body,
        http::uri::{Authority, Parts, Scheme},
        Request, Uri,
    };
    use log::{debug, error, trace};
    use std::{
        net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
        str::FromStr,
    };

    pub fn get_host_addr(req: &mut Request<body::Incoming>) -> Option<Address> {
        match get_host_addr_from_authority(req) {
            Some(addr) => Some(addr),
            None => get_host_addr_from_header(req),
        }
    }

    fn get_host_addr_from_authority(req: &mut Request<body::Incoming>) -> Option<Address> {
        let authority = match req.uri().authority() {
            Some(a) => a.clone(),
            None => {
                error!("URI {} doesn't have authority", req.uri());
                return None;
            }
        };

        match parse_host_from_authority(req.uri().scheme_str(), &authority) {
            Some(host) => {
                trace!(
                    "{:?} {} URI {} got host {} from authority",
                    req.version(),
                    req.method(),
                    req.uri(),
                    host,
                );

                // Use only path and query URI for outbound request to eliminate
                // possible 414 "URI Too Long" http error.
                if let Some(parse_and_query) = req.uri().path_and_query() {
                    let mut new_uri_parts: Parts = Parts::default();
                    new_uri_parts.path_and_query = Some(parse_and_query.clone());
                    match Uri::from_parts(new_uri_parts) {
                        Ok(uri) => {
                            debug!("Reassembled URI {} from authority, new value: {}", req.uri(), uri);
                            *req.uri_mut() = uri;
                        }
                        Err(_) => error!("Failed to reassemble URI {} from authority", req.uri()),
                    };
                }

                Some(host)
            }
            None => {
                error!(
                    "{:?} {} URI {} authority {} is invalid",
                    req.version(),
                    req.method(),
                    req.uri(),
                    authority
                );

                None
            }
        }
    }

    fn get_host_addr_from_header(req: &mut Request<body::Incoming>) -> Option<Address> {
        let host_header_value: &str = match req.headers().get("Host") {
            Some(host) => match host.to_str() {
                Ok(s) => s,
                Err(_) => {
                    error!(
                        "{:?} {} URI {} \"Host\" header invalid encoding, value: {:?}",
                        req.version(),
                        req.method(),
                        req.uri(),
                        host
                    );
                    return None;
                }
            },
            None => {
                error!(
                    "{:?} {} URI {} doesn't have valid host and missing the \"Host\" header",
                    req.version(),
                    req.method(),
                    req.uri()
                );
                return None;
            }
        };

        match Authority::from_str(host_header_value) {
            Ok(authority) => match parse_host_from_authority(req.uri().scheme_str(), &authority) {
                Some(host) => {
                    trace!(
                        "{:?} {} URI {} got host from header: {}",
                        req.version(),
                        req.method(),
                        req.uri(),
                        host
                    );

                    if reassemble_uri(req.uri_mut(), authority).is_err() {
                        error!("Failed to reassemble URI {} from \"Host\"", req.uri());
                    }

                    debug!("Reassembled URI from \"Host\", {}", req.uri());

                    Some(host)
                }
                None => {
                    error!(
                        "{:?} {} URI {} \"Host\" header invalid, value: {}",
                        req.version(),
                        req.method(),
                        req.uri(),
                        host_header_value
                    );

                    None
                }
            },
            Err(..) => {
                error!(
                    "{:?} {} URI {} \"Host\" header is not an Authority, value: {:?}",
                    req.version(),
                    req.method(),
                    req.uri(),
                    host_header_value
                );

                None
            }
        }
    }

    fn parse_host_from_authority(scheme_str: Option<&str>, authority: &Authority) -> Option<Address> {
        // RFC7230 indicates that we should ignore userinfo
        // https://tools.ietf.org/html/rfc7230#section-5.3.3

        // Check if URI has port
        let port = match authority.port_u16() {
            Some(port) => port,
            None => {
                match scheme_str {
                    None => 80, // Assume it is http
                    Some("http") => 80,
                    Some("https") => 443,
                    _ => return None, // Not supported
                }
            }
        };

        let host_str = authority.host();

        // RFC3986 indicates that IPv6 address should be wrapped in [ and ]
        // https://tools.ietf.org/html/rfc3986#section-3.2.2
        //
        // Example: [::1] without port
        if host_str.starts_with('[') && host_str.ends_with(']') {
            // Must be a IPv6 address
            let addr = &host_str[1..host_str.len() - 1];
            match addr.parse::<Ipv6Addr>() {
                Ok(ipv6) => Some(ipv6_socket_address!(ipv6, port)),
                // Ignore invalid IPv6 address
                Err(..) => None,
            }
        } else {
            // It must be a IPv4 address
            match host_str.parse::<Ipv4Addr>() {
                Ok(ipv4) => Some(ipv4_socket_address!(ipv4, port)),
                // Should be a domain name, or a invalid IP address.
                // Let DNS deal with it.
                Err(..) => Some(Address::DomainName(host_str.to_owned(), port)),
            }
        }
    }

    fn reassemble_uri(uri: &mut Uri, authority: Authority) -> Result<()> {
        // Reassemble URI
        let mut parts = uri.clone().into_parts();
        if parts.scheme.is_none() {
            // Use http as default.
            parts.scheme = Some(Scheme::HTTP);
        }
        parts.authority = Some(authority.clone());

        // Replaces URI
        *uri = Uri::from_parts(parts)?;

        Ok(())
    }
}
