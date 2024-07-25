use crate::net::tcp;
use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    body::{self},
    server::conn::http1,
    service::service_fn,
    Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use log::{debug, info, trace};
use std::{
    convert::Infallible,
    net::{SocketAddr, ToSocketAddrs},
};
use tokio::net::TcpListener;

pub struct LurkHttpEndpoint {
    addr: SocketAddr,
}

impl LurkHttpEndpoint {
    pub fn new(addr: impl ToSocketAddrs) -> LurkHttpEndpoint {
        LurkHttpEndpoint {
            addr: tcp::resolve_sockaddr(addr),
        }
    }

    /// Synchronously serve incoming HTTP requests.
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        info!("HTTP endpoint is listening on {}", self.addr);

        // Create HTTP server builder.
        let http_builder = http1::Builder::new();

        loop {
            let (tcp_stream, client_addr) = listener.accept().await?;
            let io = TokioIo::new(tcp_stream);

            trace!("Handling incoming HTTP request from {}", client_addr);
            http_builder
                .serve_connection(io, service_fn(LurkHttpEndpoint::request_handler))
                .await?;
        }
    }

    async fn request_handler(req: Request<body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
        debug!("Handling incoming {req:?}");

        let response = Response::builder();
        let response = match req.uri().path() {
            "/healthcheck" => response.status(StatusCode::OK).body(Full::new(Bytes::new())),
            _ => response.status(StatusCode::NOT_IMPLEMENTED).body(Full::new(Bytes::new())),
        };

        Ok(response.unwrap())
    }
}
