use crate::common::error::{LurkError, Unsupported};
use anyhow::{anyhow, bail, Result};
use bytes::BufMut;
use std::{
    fmt::Display,
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};
use tokio::{io::AsyncReadExt, net::lookup_host};

macro_rules! ipv4_socket_address {
    ($ipv4:expr, $port:expr) => {
        Address::SocketAddress(SocketAddr::V4(SocketAddrV4::new($ipv4, $port)))
    };
}

pub(crate) use ipv4_socket_address;

#[repr(u8)]
#[rustfmt::skip]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Address {
    SocketAddress(SocketAddr),
    DomainName(String, u16)
}

impl Address {
    pub async fn to_socket_addr(&self) -> Result<SocketAddr> {
        match self {
            Address::SocketAddress(sock_addr) => Ok(*sock_addr),
            Address::DomainName(hostname, port) => {
                // Resolve by means of builtin tokio DNS resolver
                let resolved_names = lookup_host(format!("{hostname:}:{port:}")).await?;
                // Take first found
                resolved_names
                    .into_iter()
                    .nth(0)
                    .ok_or(anyhow!(LurkError::UnresolvedDomainName(hostname.to_string())))
            }
        }
    }

    pub async fn read_ipv4<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        let ipv4 = std::net::Ipv4Addr::from(stream.read_u32().await?);
        let port = stream.read_u16().await?;

        Ok(ipv4_socket_address!(ipv4, port))
    }

    #[allow(unused_variables)]
    pub async fn read_ipv6<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        bail!(LurkError::Unsupported(Unsupported::IPv6Address))
    }

    pub async fn read_domain_name<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        let len = stream.read_u8().await?;
        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).await?;

        let name = String::from_utf8(buf).map_err(LurkError::DomainNameDecodingFailed)?;
        let port = stream.read_u16().await?;

        Ok(Address::DomainName(name, port))
    }

    pub fn write_ipv4<T: BufMut>(bytes: &mut T, ipv4_addr: &SocketAddrV4) {
        bytes.put_slice(&ipv4_addr.ip().octets());
        bytes.put_u16(ipv4_addr.port());
    }

    #[allow(unused_variables)]
    pub fn write_ipv6<T: BufMut>(bytes: &mut T, ipv6_addr: &SocketAddrV6) {
        todo!("Writing of IPv6 is not implemented")
    }

    #[allow(unused_variables)]
    pub fn write_domain_name<T: BufMut>(bytes: &mut T, name: &str, port: &u16) {
        todo!("Writing of domain names is not implemented")
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Address::SocketAddress(sock) => write!(f, "{sock:}"),
            Address::DomainName(name, port) => write!(f, "{name:}:{port:}"),
        }
    }
}
