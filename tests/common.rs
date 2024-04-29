use log::{debug, LevelFilter};
use log4rs_test_utils::test_logging::init_logging_once_for;
use lurk::server::LurkServer;
use reqwest::Proxy;
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::sleep,
};

pub const TCP_OPENED_CONN_LIMIT: usize = 1024;

pub fn init_logging() {
    init_logging_once_for(None, LevelFilter::Trace, "{h({({l}):5.5})} [{M}] {f}:{L}: {m}{n}");
}

/// Spawn Lurk proxy instance.
pub async fn spawn_lurk_server(addr: SocketAddr) -> tokio::task::JoinHandle<()> {
    // Run proxy
    let handle = tokio::spawn(async move {
        LurkServer::new(addr, TCP_OPENED_CONN_LIMIT)
            .run()
            .await
            .expect("Error during proxy server run")
    });

    // Yeild execution untill server binds
    tokio::task::yield_now().await;

    handle
}

/// Spawn TCP "Echo" server instance. It simply returns back all received data.
pub async fn spawn_echo_server(bind_addr: SocketAddr) -> tokio::task::JoinHandle<()> {
    let handle = tokio::spawn(async move {
        let listener = TcpListener::bind(bind_addr)
            .await
            .expect("Echo server should bind to specified address");

        debug!("[EchoServer] Bind listener to {bind_addr:}");

        while let Ok((mut stream, addr)) = listener.accept().await {
            debug!("[EchoServer] Accepted connection from {addr:}");
            tokio::spawn(async move {
                loop {
                    let mut read = [0; 1028];
                    match stream.read(&mut read).await {
                        Ok(n) => {
                            if n == 0 {
                                debug!("[EchoServer] Connection closed with {addr:}");
                                break;
                            } else {
                                debug!("[EchoServer] Received {n:} bytes from {addr:}");
                            }
                            stream.write_all(&read[0..n]).await.expect("Expect written bytes to stream");
                            debug!("[EchoServer] Written {n:} bytes to {addr:}")
                        }
                        Err(err) => {
                            panic!("{}", err);
                        }
                    }
                }
            });
        }
    });

    // Yeild execution untill server binds
    tokio::task::yield_now().await;

    handle
}

pub fn socks5_proxy(addr: SocketAddr) -> Proxy {
    Proxy::http(format!("socks5://{}", addr)).unwrap()
}

/// Establish connection with passed <code>endpoint</code> through <code>socks5_proxy</code>. Then send
/// data with length specified in <code>data_len</code> and expect it to be fully returned by the endpoint.
pub async fn ping_pong_data_through_socks5(endpoint: SocketAddr, socks5_proxy: SocketAddr, data_len: usize) {
    // Create TCP stream.
    let mut socks5_stream = TcpStream::connect(socks5_proxy)
        .await
        .expect("Expect successful TCP connection established with proxy");

    // Establish SOCKS5 connection over TCP stream.
    async_socks5::connect(&mut socks5_stream, endpoint, None)
        .await
        .expect("Expect successfully established SOCKS5 connection");

    // Write generated buffer.
    let write_buff = utils::generate_data(data_len);
    socks5_stream.write_all(&write_buff).await.expect("Expect all data to be written");

    // Expect it to be fully received back.
    let mut read_buff = vec![0u8; data_len];
    socks5_stream.read_exact(&mut read_buff).await.expect("Expect all data to be read");

    // Shutdown write direction.
    socks5_stream.shutdown().await.expect("Expect successful TCP stream shutdown");

    // Check that written and read data are equal.
    utils::assert_eq_vectors(&write_buff, &read_buff);

    // 1 second sleep
    sleep(Duration::from_millis(1000)).await;
}

mod utils {
    use std::fmt::Debug;

    use rand::Rng;

    pub fn assert_eq_vectors<T: Eq + Debug>(expected: &Vec<T>, actual: &Vec<T>) {
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

    pub fn generate_data(len: usize) -> Vec<u8> {
        let v = vec![0u8; len];
        let mut rng = rand::thread_rng();

        v.iter().map(|_| rng.gen::<u8>()).collect()
    }
}
