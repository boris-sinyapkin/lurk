use crate::{net::tcp, server::LurkServer};
use anyhow::Result;
use bytes::Bytes;
use chrono::{DateTime, TimeDelta, Utc};
use http_body_util::Full;
use hyper::{
    body::{self},
    server::conn::http1,
    service::Service,
    Request, Response, StatusCode,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use log::{debug, error, info, log_enabled, trace};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSeconds};
use std::{
    future::Future,
    net::{SocketAddr, ToSocketAddrs},
    pin::Pin,
    sync::Arc,
};
use tokio::net::TcpListener;

pub struct LurkHttpEndpoint {
    addr: SocketAddr,
    service: LurkHttpService,
}

impl LurkHttpEndpoint {
    pub fn new(addr: impl ToSocketAddrs, node: Arc<LurkServer>) -> LurkHttpEndpoint {
        LurkHttpEndpoint {
            addr: tcp::resolve_sockaddr(addr),
            service: LurkHttpService { node },
        }
    }

    /// Asynchronously serve incoming HTTP requests.
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        info!("HTTP endpoint is listening on {}", self.addr);

        loop {
            let (tcp_stream, client_addr) = listener.accept().await?;
            let io = TokioIo::new(tcp_stream);
            let service = self.service.clone();

            debug!("Incoming HTTP request from {}", client_addr);

            tokio::spawn(async move {
                // Handle the connection from the client using HTTP1 and pass any
                // HTTP requests received on that connection to the service.
                if let Err(err) = http1::Builder::new().timer(TokioTimer::new()).serve_connection(io, service).await {
                    error!("Error occured while handling HTTP request from {client_addr:}: {err:?}");
                }
            });
        }
    }
}

#[derive(Clone)]
struct LurkHttpService {
    node: Arc<LurkServer>,
}

impl Service<Request<body::Incoming>> for LurkHttpService {
    type Error = anyhow::Error;
    type Response = Response<Full<Bytes>>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, request: Request<body::Incoming>) -> Self::Future {
        let uri_path = request.uri().path();

        // Dump full request data if trace is enabled
        if log_enabled!(log::Level::Trace) {
            trace!("{:?}", request);
        } else {
            info!("{:?} {} '{}'", request.version(), request.method(), uri_path);
        }

        let response = match uri_path {
            "/healthcheck" => {
                let node_status = LurkNodeStatus::build(&self.node);
                trace!("Response to '{uri_path}': {node_status:?}");
                Response::builder()
                    .header("Content-Type", "application/json")
                    .body(node_status.serialize_as_body_chunk())
            }
            _ => Response::builder()
                .status(StatusCode::NOT_IMPLEMENTED)
                .body(Full::new(Bytes::new())),
        };

        Box::pin(async { Ok(response.unwrap()) })
    }
}

/// Structure describing node health status sent as HTTP response.
#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
struct LurkNodeStatus {
    /// Timespan between "started" and "current" timestamps.
    #[serde_as(as = "Option<DurationSeconds<i64>>")]
    uptime_secs: Option<TimeDelta>,

    /// UTC timestamp made when node started to accept connections.
    started_utc_ts: Option<DateTime<Utc>>,
}

impl LurkNodeStatus {
    /// Fill status structure depending on the information retrived
    /// from input node.
    fn build(node: &LurkServer) -> LurkNodeStatus {
        let node_stats = node.get_stats();
        let mut uptime_secs = None;
        let mut started_utc_ts = None;

        if node_stats.is_server_started() {
            uptime_secs = Some(node_stats.get_uptime());
            started_utc_ts = Some(node_stats.get_started_utc_timestamp());
        }

        LurkNodeStatus {
            uptime_secs,
            started_utc_ts,
        }
    }

    /// Try to serialize input data. Returns serialized bytes on succes.
    /// On failure, empty bytes is returned.
    fn serialize_as_body_chunk(&self) -> Full<Bytes> {
        let bytes = match serde_json::to_string(&self) {
            Ok(bytes) => Bytes::from(bytes),
            Err(err) => {
                error!(
                    "Error occured during body serialization: {err:?}.
                    Empty body has been returned."
                );
                Bytes::new()
            }
        };

        Full::new(bytes)
    }
}
