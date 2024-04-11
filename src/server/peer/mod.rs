use crate::io::{stream::LurkStreamWrapper, LurkRequestRead, LurkResponseWrite};
use anyhow::{bail, Error, Result};
use std::{
    fmt::Display,
    io::{self},
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

mod auth;
pub mod handlers;

pub type LurkTcpPeer = LurkPeer<LurkStreamWrapper<TcpStream>>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LurkPeerType {
    Socks5Peer = 0x05,
}

impl LurkPeerType {
    pub async fn from_tcp_stream(tcp_stream: &TcpStream) -> Result<LurkPeerType> {
        let mut buff = [0u8; 1];
        let peeked_bytes = match tcp_stream.peek(&mut buff).await {
            Ok(n) => n,
            Err(err) => bail!(err),
        };

        if peeked_bytes == 1 {
            match buff[0] {
                0x05 => Ok(LurkPeerType::Socks5Peer),
                t => bail!(Error::msg(format!("Unknown peer type {t:#04x}"))),
            }
        } else {
            bail!(io::Error::new(io::ErrorKind::UnexpectedEof, "unable to peek peer type from stream"))
        }
    }
}

impl Display for LurkPeerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LurkPeerType::Socks5Peer => write!(f, "SOCKS5"),
        }
    }
}

pub struct LurkPeer<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin,
{
    addr: SocketAddr,
    stream: S,
    peer_type: LurkPeerType,
}

impl<S> LurkPeer<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin + DerefMut,
    <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S, addr: SocketAddr, peer_type: LurkPeerType) -> LurkPeer<S> {
        LurkPeer { stream, addr, peer_type }
    }

    pub fn peer_type(&self) -> LurkPeerType {
        self.peer_type
    }
}

impl<S> Display for LurkPeer<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.addr)
    }
}

#[cfg(test)]
mod tests {}
