use crate::net::tcp::connection::{LurkTcpConnectionHandler, LurkTcpConnectionLabel};
use anyhow::{bail, Result};
use http::LurkHttpHandler;
use socks5::LurkSocks5Handler;

mod http;
mod socks5;

pub fn create_tcp_connection_handler(label: &LurkTcpConnectionLabel) -> Result<Box<dyn LurkTcpConnectionHandler>> {
    match label {
        LurkTcpConnectionLabel::Http | LurkTcpConnectionLabel::HttpSecure => Ok(Box::new(LurkHttpHandler {})),
        LurkTcpConnectionLabel::Socks5 => Ok(Box::new(LurkSocks5Handler {})),
        LurkTcpConnectionLabel::Unknown(_) => bail!("Unknown TCP connection"),
    }
}
