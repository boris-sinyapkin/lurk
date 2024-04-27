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

    use anyhow::Result;
    use std::net::SocketAddr;
    use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

    /// Custom implementation of TCP listener.
    pub struct LurkTcpListener {
        inner: TcpListener,
    }

    impl LurkTcpListener {
        pub async fn bind(addr: impl ToSocketAddrs) -> Result<LurkTcpListener> {
            Ok(LurkTcpListener {
                inner: TcpListener::bind(&addr).await?,
            })
        }

        pub async fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
            self.inner.accept().await.map_err(anyhow::Error::from)
        }
    }
}
