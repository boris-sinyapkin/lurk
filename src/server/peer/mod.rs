use crate::io::{stream::LurkStreamWrapper, LurkRequestRead, LurkResponseWrite};
use std::{
    fmt::Display,
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub mod auth;
pub mod handlers;

pub type LurkTcpPeer = LurkPeer<LurkStreamWrapper<TcpStream>>;

pub struct LurkPeer<S>
where
    S: LurkRequestRead + LurkResponseWrite + Unpin,
{
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
