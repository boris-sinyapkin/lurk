use crate::{
    io::tunnel::LurkTunnel,
    net::tcp::connection::{LurkTcpConnection, LurkTcpConnectionHandler},
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
