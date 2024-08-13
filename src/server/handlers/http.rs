use crate::{
    io::tunnel::LurkTunnel,
    net::tcp::{
        self,
        connection::{LurkTcpConnection, LurkTcpConnectionHandler},
    },
};
use anyhow::{anyhow, Result};
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

        if request.method() == Method::CONNECT {
            let addr_str = match request.uri().authority() {
                Some(str) => str.to_string(),
                None => {
                    error!("CONNECT host is not socket addr: {:?}", request.uri());
                    return Ok(Self::bad_request());
                }
            };

            let mut outbound = match tcp::establish_tcp_connection(addr_str).await {
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
            let host = request.uri().host().ok_or(anyhow!("HTTP request has no host"))?;
            let port = request.uri().port_u16().unwrap_or(80);

            let stream = TcpStream::connect((host, port)).await?;
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

            // Use only path for outbound request to eliminate possible 414 "URI Too Long" http error.
            *request.uri_mut() = request.uri().path().parse()?;

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
