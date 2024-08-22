use super::AsyncListener;
use anyhow::Result;
use futures::{future::poll_fn, ready, FutureExt};
use log::{debug, error, trace};
use std::{
    future::Future,
    io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::{TcpListener, TcpStream};

/*
 * TCP Echo server listener
 */
pub struct TcpEchoServer {
    state: TcpEchoServerState,
    inner: TcpListener,
}

enum TcpEchoServerState {
    AcceptingConnection,
    HandlingConnection(Connection),
}

impl TcpEchoServer {
    pub async fn bind(addr: SocketAddr) -> TcpEchoServer {
        debug!("[TcpEchoServerListener] Binding TCP echo server to {addr}");
        TcpEchoServer {
            inner: TcpListener::bind(addr).await.unwrap(),
            state: TcpEchoServerState::AcceptingConnection,
        }
    }
}

struct Connection {
    addr: SocketAddr,
    state: ConnectionState,
    stream: TcpStream,
}

enum ConnectionState {
    ReadingStream,
    WritingStream(Vec<u8>),
    Closed,
}

impl Connection {
    fn new(stream: TcpStream, addr: SocketAddr) -> Connection {
        Connection {
            addr,
            stream,
            state: ConnectionState::ReadingStream,
        }
    }
}

impl Future for Connection {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match &self.state {
                ConnectionState::ReadingStream => {
                    ready!(self.stream.poll_read_ready(cx))?;

                    let mut buf: Vec<u8> = vec![0u8; 1024];
                    match self.stream.try_read(&mut buf) {
                        Ok(0) => {
                            debug!("[TcpEchoServerListener] Received EOF from {}", self.addr);
                            self.state = ConnectionState::Closed;
                        }
                        Ok(n) => {
                            debug!("[TcpEchoServerListener] Received {n:} bytes from {}", self.addr);
                            self.state = ConnectionState::WritingStream(buf);
                        }
                        Err(e) => {
                            if e.kind() == io::ErrorKind::WouldBlock {
                                trace!("[TcpEchoServerListener] Re-try reading");
                                continue;
                            }
                            return Poll::Ready(Err(e.into()));
                        }
                    }
                }
                ConnectionState::WritingStream(buf) => {
                    ready!(self.stream.poll_write_ready(cx))?;

                    match self.stream.try_write(buf) {
                        Ok(n) => {
                            debug!("[TcpEchoServerListener] Wrote {n:} bytes to {}", self.addr);
                            self.state = ConnectionState::ReadingStream;
                            continue;
                        }
                        Err(e) => {
                            if e.kind() == io::ErrorKind::WouldBlock {
                                trace!("[TcpEchoServerListener] Re-try writing");
                                continue;
                            }
                            return Poll::Ready(Err(e.into()));
                        }
                    }
                }
                ConnectionState::Closed => {
                    debug!("[TcpEchoServerListener] Connection closed with {}", self.addr);
                    return Poll::Ready(Ok(()));
                }
            };
        }
    }
}

impl AsyncListener for TcpEchoServer {
    fn listen(&mut self) -> impl Future<Output = Result<()>> + Send {
        poll_fn(move |cx| loop {
            match &mut self.state {
                TcpEchoServerState::AcceptingConnection => match ready!(self.inner.poll_accept(cx)) {
                    Ok((stream, addr)) => {
                        debug!("[TcpEchoServerListener] Accepted connection from {addr:}");
                        self.state = TcpEchoServerState::HandlingConnection(Connection::new(stream, addr));
                    }
                    Err(err) => {
                        error!("[TcpEchoServerListener] Error happened while accepting TCP connection: {}", err);
                    }
                },
                TcpEchoServerState::HandlingConnection(conn) => {
                    if let Err(err) = ready!(conn.poll_unpin(cx)) {
                        error!(
                            "[TcpEchoServerListener] Connection with address {} has finished with error: {err}",
                            conn.addr
                        );
                    }
                    self.state = TcpEchoServerState::AcceptingConnection;
                }
            }
        })
    }

    fn name(&self) -> &'static str {
        "TCP echo server"
    }
}
