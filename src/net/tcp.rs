use super::Address;
use anyhow::Result;
use log::{debug, trace};
use socket2::{SockRef, TcpKeepalive};
use std::net::{SocketAddr, ToSocketAddrs};
use tokio::net::TcpStream;

/// Different TCP connection options.
///
/// **Fields**:
/// * ```keep_alive``` - setting for TCP keepalive procedure
///
///
pub struct TcpConnectionOptions {
    keep_alive: Option<TcpKeepalive>,
}

impl TcpConnectionOptions {
    pub fn new() -> TcpConnectionOptions {
        TcpConnectionOptions { keep_alive: None }
    }

    pub fn set_keepalive(&mut self, keep_alive: TcpKeepalive) -> &mut TcpConnectionOptions {
        debug_assert!(self.keep_alive.is_none(), "should be unset");
        self.keep_alive = Some(keep_alive);
        self
    }

    pub fn apply_to(&self, tcp_stream: &mut TcpStream) -> Result<()> {
        let tcp_sock_ref = SockRef::from(&tcp_stream);

        if let Some(keep_alive) = &self.keep_alive {
            tcp_sock_ref.set_tcp_keepalive(keep_alive)?;
        }

        Ok(())
    }
}

/// Establish TCP connection with passed ```endpoint```.
///
/// Input ```tcp_opts``` are applied to created TCP socket right after stream creation.
pub async fn establish_tcp_connection_with_opts(endpoint: &Address, tcp_opts: &TcpConnectionOptions) -> Result<TcpStream> {
    // Resolve endpoint address.
    trace!("Endpoint address {} resolution: ... ", endpoint);
    let resolved = endpoint.to_socket_addr().await?;
    trace!("Endpoint address {} resolution: SUCCESS with {}", endpoint, resolved);

    // Establish TCP connection with the endpoint.
    debug!("TCP connection establishment with the endpoint {}: ... ", endpoint);
    let mut tcp_stream = TcpStream::connect(resolved).await.map_err(anyhow::Error::from)?;
    debug!("TCP connection establishment with the endpoint {}: SUCCESS", endpoint);

    // Apply passed options to created TCP stream.
    tcp_opts.apply_to(&mut tcp_stream)?;

    Ok(tcp_stream)
}

pub fn resolve_sockaddr(addr: impl ToSocketAddrs) -> SocketAddr {
    // Return first resolved socket address
    addr.to_socket_addrs().unwrap().next().expect("Expect benign address to resolve")
}

pub mod listener {

    use super::connection::{LurkTcpConnection, LurkTcpConnectionFactory, LurkTcpConnectionLabel};
    use anyhow::Result;
    use socket2::{Domain, Socket, Type};
    use std::net::{SocketAddr, ToSocketAddrs};
    use tokio::net::TcpListener;
    use tokio_stream::{wrappers::TcpListenerStream, StreamExt};

    const TCP_LISTEN_BACKLOG: i32 = 1024;

    /// Custom implementation of TCP listener.
    #[allow(dead_code)]
    pub struct LurkTcpListener {
        incoming: TcpListenerStream,
        factory: LurkTcpConnectionFactory,
        local_addr: SocketAddr,
    }

    impl LurkTcpListener {
        /// Binds TCP listener to passed `addr`.
        ///
        pub async fn bind(addr: impl ToSocketAddrs) -> Result<LurkTcpListener> {
            let bind_addr = super::resolve_sockaddr(addr);

            // Create TCP socket
            let socket = Socket::new(Domain::for_address(bind_addr), Type::STREAM, None)?;

            // Bind TCP socket and mark it ready to accept incoming connections
            socket.bind(&bind_addr.into())?;
            socket.listen(TCP_LISTEN_BACKLOG)?;

            // Set TCP options
            socket.set_nonblocking(true)?;

            // Create tokio TCP listener from TCP socket
            let inner: TcpListener = TcpListener::from_std(socket.into())?;
            let local_addr = inner.local_addr()?;

            // Create backpressure limit and supply the receiver to the created stream.
            let incoming = TcpListenerStream::new(inner);

            Ok(LurkTcpListener {
                incoming,
                factory: LurkTcpConnectionFactory::new(),
                local_addr,
            })
        }

        /// Accept incoming TCP connection.
        pub async fn accept(&mut self) -> Result<LurkTcpConnection> {
            let err_msg: &str = "Incoming TCP listener should never return empty option";
            let tcp_stream = self.incoming.next().await.expect(err_msg)?;
            let tcp_label = LurkTcpConnectionLabel::from_tcp_stream(&tcp_stream).await?;

            self.factory.create_connection(tcp_stream, tcp_label)
        }

        /// Returns local address that this listener is binded to.
        #[allow(dead_code)]
        pub fn local_addr(&self) -> SocketAddr {
            self.local_addr
        }
    }

    #[cfg(test)]
    mod tests {

        use super::*;
        use futures::{stream::FuturesUnordered, TryFutureExt};
        use std::time::Duration;
        use tokio::{
            io::AsyncWriteExt,
            net::TcpStream,
            time::{sleep, timeout},
        };

        // :0 tells the OS to pick an open port.
        const TEST_BIND_IPV4: &str = "127.0.0.1:0";

        /// This tests backpressure limit set on listener.
        /// Number of connections intentionally exceeds the limit. Thus listener
        /// should put on hold some of them and handle only allowed number of
        /// them in parallel.
        #[ignore]
        #[tokio::test]
        async fn limit_tcp_connections() {
            let conn_limit = 5;
            let num_clients = 20;

            let mut listener = LurkTcpListener::bind(TEST_BIND_IPV4).await.expect("Expect binded listener");
            let listener_addr = listener.local_addr();

            let client_tasks: FuturesUnordered<_> = (0..num_clients)
                .map(|_| async move {
                    TcpStream::connect(listener_addr)
                        .and_then(|mut s| async move { s.write_all(&[0x05]).await })
                        .await
                        .unwrap()
                })
                .collect();

            // Await all clients to complete.
            client_tasks.collect::<()>().await;

            // We have to handle all clients, but only `conn_limit`
            // could be handled in parallel.
            for _ in 0..num_clients {
                let conn = timeout(Duration::from_secs(2), listener.accept())
                    .await
                    .expect("Expect accepted connection before expired timeout")
                    .expect("Expect accepted TCP connection");

                assert_eq!(LurkTcpConnectionLabel::SOCKS5, conn.label());
                assert!(
                    listener.factory.get_active_tokens() <= conn_limit,
                    "Number of opened connections must not exceed the limit"
                );

                tokio::spawn(async move {
                    // Some client handling ...
                    sleep(Duration::from_millis(300)).await;
                    // Drop the connection after sleep, hence one
                    // slot should become available for the next client
                    drop(conn)
                });
            }
        }
    }
}

pub mod connection {

    use crate::io::stream::{LurkStream, LurkTcpStream};
    use anyhow::{bail, Result};
    use std::{fmt::Display, io, net::SocketAddr};
    use tokio::net::TcpStream;

    /// Label that describes the TCP connection.
    ///
    /// Once new TCP client is connected, ```LurkTcpListener``` peeks the stream
    /// and checks the values inside. If the value in unknown, the connection is skipped.
    ///
    #[derive(Debug, Clone, Copy, PartialEq)]
    #[repr(u8)]
    pub enum LurkTcpConnectionLabel {
        /// Traffic of TCP connection belongs to proxy SOCKS5 protocol
        SOCKS5 = 0x05,

        /// Unknown traffic
        Unknown(u8),
    }

    impl LurkTcpConnectionLabel {
        /// Peeks input TCP stream and retrieves the first read byte value.
        /// This byte is mapped to the one of known values ```LurkTcpConnectionLabel```.
        pub async fn from_tcp_stream(tcp_stream: &TcpStream) -> Result<LurkTcpConnectionLabel> {
            let mut buff = [0u8; 1];
            let peeked_bytes = match tcp_stream.peek(&mut buff).await {
                Ok(n) => n,
                Err(err) => bail!(err),
            };

            if peeked_bytes == 1 {
                let label = match buff[0] {
                    0x05 => LurkTcpConnectionLabel::SOCKS5,
                    v => LurkTcpConnectionLabel::Unknown(v),
                };

                Ok(label)
            } else {
                bail!(io::ErrorKind::UnexpectedEof)
            }
        }
    }

    impl Display for LurkTcpConnectionLabel {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                LurkTcpConnectionLabel::SOCKS5 => write!(f, "SOCKS5"),
                LurkTcpConnectionLabel::Unknown(l) => write!(f, "Unknown TCP label {l:#04x}"),
            }
        }
    }

    /// Factory that produces new TCP connection instances.
    ///
    /// For each new instance, factory uses backpressure 'sender' to create the token that
    /// should be destroyed on TCP connection drop.
    ///
    pub struct LurkTcpConnectionFactory {}

    impl LurkTcpConnectionFactory {
        pub fn new() -> LurkTcpConnectionFactory {
            LurkTcpConnectionFactory {}
        }

        /// Returns the number of currently active tokens.
        #[allow(dead_code)]
        pub fn get_active_tokens(&self) -> usize {
            0
        }

        pub fn create_connection(&self, tcp_stream: TcpStream, label: LurkTcpConnectionLabel) -> Result<LurkTcpConnection> {
            // Wrap raw TcpStream to the stream wrapper and generate new backpressure token
            // that must be dropped on connection destruction.
            LurkTcpConnection::new(tcp_stream, label)
        }
    }

    pub struct LurkTcpConnection {
        /// Lurk wrapper of TcpStream
        stream: LurkTcpStream,
        /// Label describing traffic in this TCP connection
        label: LurkTcpConnectionLabel,
        /// Remote address that this connection is connected to
        peer_addr: SocketAddr,
        /// Local address that this connection is bound to
        local_addr: SocketAddr,
    }

    impl LurkTcpConnection {
        fn new(tcp_stream: TcpStream, label: LurkTcpConnectionLabel) -> Result<LurkTcpConnection> {
            Ok(LurkTcpConnection {
                peer_addr: tcp_stream.peer_addr()?,
                local_addr: tcp_stream.local_addr()?,
                stream: LurkStream::new(tcp_stream),
                label,
            })
        }

        pub fn peer_addr(&self) -> SocketAddr {
            self.peer_addr
        }

        pub fn local_addr(&self) -> SocketAddr {
            self.local_addr
        }

        pub fn label(&self) -> LurkTcpConnectionLabel {
            self.label
        }

        pub fn stream_mut(&mut self) -> &mut LurkTcpStream {
            &mut self.stream
        }
    }

    #[cfg(test)]
    mod tests {

        use super::*;
        use futures::TryFutureExt;
        use tokio::{io::AsyncWriteExt, net::TcpListener};

        // :0 tells the OS to pick an open port.
        const TEST_BIND_IPV4: &str = "127.0.0.1:0";

        #[tokio::test]
        async fn extract_tcp_conn_label() {
            // :0 tells the OS to pick an open port.
            let listener = TcpListener::bind(TEST_BIND_IPV4).await.expect("Expect binded listener");
            let addr = listener.local_addr().unwrap();

            {
                // Write known label (SOCKS5)
                TcpStream::connect(addr)
                    .and_then(|mut s| async move { s.write_all(&[0x05]).await })
                    .await
                    .unwrap();
            }

            listener
                .accept()
                .and_then(|(s, _)| async move {
                    let label = LurkTcpConnectionLabel::from_tcp_stream(&s).await.unwrap();
                    assert_eq!(LurkTcpConnectionLabel::SOCKS5, label);
                    Ok(())
                })
                .await
                .unwrap();

            {
                // Write unknown label
                TcpStream::connect(addr)
                    .and_then(|mut s| async move { s.write_all(&[0xFF]).await })
                    .await
                    .unwrap();
            }

            listener
                .accept()
                .and_then(|(s, _)| async move {
                    let label = LurkTcpConnectionLabel::from_tcp_stream(&s).await.unwrap();
                    assert_eq!(LurkTcpConnectionLabel::Unknown(0xFF), label);
                    Ok(())
                })
                .await
                .unwrap();
        }
    }
}
