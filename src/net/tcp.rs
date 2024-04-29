use super::Address;
use anyhow::Result;
use log::{debug, trace};
use socket2::{SockRef, TcpKeepalive};
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

pub mod listener {

    use super::connection::{LurkTcpConnection, LurkTcpConnectionFactory, LurkTcpConnectionLabel};
    use anyhow::Result;
    use async_listen::{backpressure, backpressure::Backpressure, ListenExt};
    use tokio::net::{TcpListener, ToSocketAddrs};
    use tokio_stream::{wrappers::TcpListenerStream, StreamExt};

    /// Custom implementation of TCP listener.
    pub struct LurkTcpListener {
        incoming: Backpressure<TcpListenerStream>,
        factory: LurkTcpConnectionFactory,
    }

    impl LurkTcpListener {
        /// Binds TCP listener to passed `addr`.
        ///
        /// Argument `conn_limit` sets the limit of open TCP connections. Thus accepting of new connections
        /// on returned `LurkTcpListener` will be paused, when number of open TCP connections will reach
        /// the `conn_limit`.
        pub async fn bind(addr: impl ToSocketAddrs, conn_limit: usize) -> Result<LurkTcpListener> {
            // Bind TCP listener.
            let listener = TcpListener::bind(addr).await?;

            // Create backpressure limit and supply the receiver to the created stream.
            let (bp_tx, bp_rx) = backpressure::new(conn_limit);
            let incoming = TcpListenerStream::new(listener).apply_backpressure(bp_rx);

            Ok(LurkTcpListener {
                incoming,
                factory: LurkTcpConnectionFactory::new(bp_tx),
            })
        }

        pub async fn accept(&mut self) -> Result<LurkTcpConnection> {
            let err_msg: &str = "Incoming TCP listener should never return empty option";
            let tcp_stream = self.incoming.next().await.expect(err_msg)?;
            let tcp_label = LurkTcpConnectionLabel::from_tcp_stream(&tcp_stream).await?;

            self.factory.create_connection(tcp_stream, tcp_label)
        }
    }
}

pub mod connection {

    use crate::{
        common::error::LurkError,
        io::stream::{LurkStream, LurkTcpStream},
    };
    use anyhow::{bail, Result};
    use async_listen::backpressure::{Sender, Token};
    use std::{fmt::Display, io, net::SocketAddr};
    use tokio::net::TcpStream;

    /// Label that describes the TCP connection.
    ///
    /// Once new TCP client is connected, ```LurkTcpListener``` peeks the stream
    /// and checks the values inside. If the value in unknown, the connection is skipped.
    ///
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum LurkTcpConnectionLabel {
        /// Traffic of TCP connection belongs to proxy SOCKS5 protocol
        SOCKS5 = 0x05,
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
                match buff[0] {
                    0x05 => Ok(LurkTcpConnectionLabel::SOCKS5),
                    t => bail!(LurkError::UnknownTcpConnectionLabel(t)),
                }
            } else {
                bail!(io::ErrorKind::UnexpectedEof)
            }
        }
    }

    impl Display for LurkTcpConnectionLabel {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                LurkTcpConnectionLabel::SOCKS5 => write!(f, "SOCKS5"),
            }
        }
    }

    /// Factory that produces new TCP connection instances.
    ///
    /// For each new instance, factory uses backpressure 'sender' to create the token that
    /// should be destroyed on TCP connection drop.
    ///
    pub struct LurkTcpConnectionFactory {
        /// Backpressure sender instance.
        /// This will produce tokens for created TCP connections.
        bp_tx: Sender,
    }

    impl LurkTcpConnectionFactory {
        pub fn new(bp_tx: Sender) -> LurkTcpConnectionFactory {
            LurkTcpConnectionFactory { bp_tx }
        }

        pub fn create_connection(&self, tcp_stream: TcpStream, label: LurkTcpConnectionLabel) -> Result<LurkTcpConnection> {
            // Wrap raw TcpStream to the stream wrapper and generate new backpressure token
            // that must be dropped on connection destruction.
            Ok(LurkTcpConnection {
                peer_addr: tcp_stream.peer_addr()?,
                local_addr: tcp_stream.local_addr()?,
                stream: LurkStream::new(tcp_stream),
                _token: self.bp_tx.token(),
                label,
            })
        }
    }

    pub struct LurkTcpConnection {
        /// Lurk wrapper of TcpStream
        stream: LurkTcpStream,
        /// Backpressure token
        _token: Token,
        /// Label describing traffic in this TCP connection
        label: LurkTcpConnectionLabel,
        /// Remote address that this connection is connected to
        peer_addr: SocketAddr,
        /// Local address that this connection is bound to
        local_addr: SocketAddr,
    }

    impl LurkTcpConnection {
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
}
