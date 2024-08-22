use anyhow::Result;
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use log::{debug, LevelFilter};
use log4rs_test_utils::test_logging::init_logging_once_for;
use reqwest::Proxy;
use std::{net::SocketAddr, sync::atomic::{AtomicUsize, Ordering}};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{yield_now, JoinHandle},
};
use tokio_util::sync::CancellationToken;
use utils::assertions::assert_eq_vectors;

pub mod listeners;

pub fn init_logging() {
    init_logging_once_for(None, LevelFilter::Debug, "{h({({l}):5.5})} [{M}] {f}:{L}: {m}{n}");
}

pub fn next_available_address() -> SocketAddr {
    static PORT: AtomicUsize = AtomicUsize::new(32000);

    format!("127.0.0.1:{}", PORT.fetch_add(1, Ordering::AcqRel)).parse().unwrap()
}

// Spawns single-threaded HTTP echo server
pub async fn spawn_http_echo_server(bind_addr: SocketAddr) -> (JoinHandle<()>, CancellationToken) {
    /// This is our service handler. It receives a Request, routes on its
    /// path, and returns a Future of a Response.
    async fn echo(request: Request<Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>> {
        debug!("{:?} {} '{}'", request.version(), request.method(), request.uri().path());
        match request.uri().path() {
            // Simply echo the body back to the client.
            "/echo" => Ok(Response::builder().body(request.into_body().boxed()).unwrap()),
            // Return the 404 Not Found for other routes.
            _ => Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Empty::<Bytes>::new().map_err(|never| match never {}).boxed())
                .unwrap()),
        }
    }

    // Clients dispatching infinite loop
    async fn main_loop(listener: TcpListener) {
        loop {
            let (stream, addr) = listener
                .accept()
                .await
                .expect("[Spawned HTTP Echo Server] Failed to accept TCP connection");

            let io = TokioIo::new(stream);

            debug!("[Spawned HTTP Echo Server] Accepted new TCP connection: {}", addr);

            if let Err(err) = http1::Builder::new().serve_connection(io, service_fn(echo)).await {
                panic!("[Spawned HTTP Echo Server] Error serving HTTP connection: \"{}\"", err);
            }
        }
    }

    // Create cancellation token to track external shutdown request.
    let cancellation_token = CancellationToken::new();

    let task_token = cancellation_token.clone();
    let task_handle = tokio::spawn(async move {
        let listener = TcpListener::bind(bind_addr)
            .await
            .expect("[Spawned HTTP Echo Server] Failed to bind TCP listener");

        debug!("[Spawned HTTP Echo Server] Started. Listening on {}", bind_addr);
        tokio::select! {
            _ = main_loop(listener) => {}
            _ = task_token.cancelled() => {}
        }
        debug!("[Spawned HTTP Echo Server] Server is shutting down ...");
    });

    // Yeild execution untill server binds
    yield_now().await;

    (task_handle, cancellation_token)
}

pub fn socks5_proxy(addr: SocketAddr) -> Proxy {
    Proxy::http(format!("socks5://{}", addr)).unwrap()
}

#[allow(dead_code)]
pub fn http_proxy(addr: SocketAddr) -> Proxy {
    Proxy::http(format!("http://{}", addr)).unwrap()
}

/// Establish connection with passed <code>endpoint</code> through <code>socks5_proxy</code>. Then send
/// data  and expect it to be fully returned by the endpoint.
pub async fn ping_pong_data_through_socks5(endpoint: SocketAddr, socks5_proxy: SocketAddr) {
    // Create TCP stream.
    let mut socks5_stream = TcpStream::connect(socks5_proxy)
        .await
        .expect("Expect successful TCP connection established with proxy");

    // Establish SOCKS5 connection over TCP stream.
    async_socks5::connect(&mut socks5_stream, endpoint, None)
        .await
        .expect("Expect successfully established SOCKS5 connection");

    // Write generated buffer.
    let write_buff = utils::generate_data(1024);
    socks5_stream.write_all(&write_buff).await.expect("Expect all data to be written");

    // Expect it to be fully received back.
    let mut read_buff = vec![0u8; 1024];
    socks5_stream.read_exact(&mut read_buff).await.expect("Expect all data to be read");

    // Shutdown write direction.
    socks5_stream.shutdown().await.expect("Expect successful TCP stream shutdown");

    // Check that written and read data are equal.
    assert_eq_vectors(&write_buff, &read_buff);
}

pub mod utils {

    use rand::Rng;

    pub mod assertions {

        use std::fmt::Debug;

        pub fn assert_eq_vectors<T: Eq + Debug>(expected: &[T], actual: &[T]) {
            let matching = expected
                .iter()
                .zip(actual)
                .filter(|&(r, w)| {
                    assert_eq!(r, w);
                    r == w
                })
                .count();

            assert_eq!(expected.len(), matching, "whole buffers (write & read) should be equal");
        }
    }

    pub mod http {

        use reqwest::{Client, ClientBuilder, Proxy};

        pub fn create_http_client() -> Client {
            construct_http_client(None)
        }

        pub fn create_http_client_with_proxy(proxy: Proxy) -> Client {
            construct_http_client(Some(proxy))
        }

        fn construct_http_client(proxy: Option<Proxy>) -> Client {
            let mut builder = ClientBuilder::new();

            if let Some(p) = proxy {
                builder = builder.proxy(p);
            }

            builder.build().expect("Unable to build HTTP client")
        }
    }

    pub fn generate_data(len: usize) -> Vec<u8> {
        let v = vec![0u8; len];
        let mut rng = rand::thread_rng();

        v.iter().map(|_| rng.gen::<u8>()).collect()
    }
}
