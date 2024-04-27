use crate::io::{stream::LurkStream, LurkRequestRead, LurkResponseWrite};
use anyhow::{bail, Result};
use std::{
    fmt::Display,
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

mod auth;
pub mod handlers;

pub type LurkTcpPeer = LurkPeer<LurkStream<TcpStream>>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LurkPeerType {
    SOCKS5 = 0x05,
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
                0x05 => Ok(LurkPeerType::SOCKS5),
                t => bail!("Unknown peer type {t:#04x}"),
            }
        } else {
            bail!("Unable to identify peer type (EOF)")
        }
    }
}

impl Display for LurkPeerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LurkPeerType::SOCKS5 => write!(f, "SOCKS5"),
        }
    }
}

pub struct LurkPeer<S> {
    addr: SocketAddr,
    stream: S,
}

impl<S> LurkPeer<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin + DerefMut,
    <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S, addr: SocketAddr) -> LurkPeer<S> {
        LurkPeer { stream, addr }
    }
}

impl<S> Display for LurkPeer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.addr)
    }
}

#[cfg(test)]
mod tests {}
