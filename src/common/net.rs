use crate::common::error::LurkError;
use anyhow::{anyhow, Result};
use bytes::BufMut;
use std::{
    fmt::Display,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};
use tokio::{io::AsyncReadExt, net::lookup_host};

macro_rules! ipv4_socket_address {
    ($ipv4:expr, $port:expr) => {
        Address::SocketAddress(SocketAddr::V4(SocketAddrV4::new($ipv4, $port)))
    };
}

macro_rules! ipv6_socket_address {
    ($ipv6:expr, $port:expr) => {
        Address::SocketAddress(SocketAddr::V6(SocketAddrV6::new($ipv6, $port, 0, 0)))
    };
}

pub(crate) use ipv4_socket_address;
pub(crate) use ipv6_socket_address;

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
        let ipv4 = Ipv4Addr::from(stream.read_u32().await?);
        let port = stream.read_u16().await?;

        Ok(ipv4_socket_address!(ipv4, port))
    }

    pub async fn read_ipv6<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        let ipv6 = Ipv6Addr::from(stream.read_u128().await?);
        let port = stream.read_u16().await?;

        Ok(ipv6_socket_address!(ipv6, port))
    }

    pub async fn read_domain_name<T: AsyncReadExt + Unpin>(stream: &mut T, len: u8) -> Result<Address> {
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

    pub fn write_ipv6<T: BufMut>(bytes: &mut T, ipv6_addr: &SocketAddrV6) {
        bytes.put_slice(&ipv6_addr.ip().octets());
        bytes.put_u16(ipv6_addr.port());
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::net::Ipv4Addr;
    use tokio_test::{assert_err, assert_ok};

    #[tokio::test]
    async fn domain_to_socket_addr() {
        let resolved = Address::DomainName("www.example.com".to_owned(), 80);
        assert_ok!(resolved.to_socket_addr().await);

        let unresolved = Address::DomainName("unresolved123".to_owned(), 666);
        assert_err!(unresolved.to_socket_addr().await);
    }

    #[tokio::test]
    async fn read_address_from_stream() {
        let domain_name = "www.example.com".to_string();
        let domain_name_len = domain_name.len() as u8;
        let mut mock = tokio_test::io::Builder::new()
            .read(&[127, 0, 0, 1, 10, 10]) // ipv4
            .read(&[0, 0, 0, 0, 0, 0xff, 0xff, 0xc0, 0x0a, 0x02, 0xff, 0xca, 0x1, 0x0, 0x11, 0xff, 10, 10]) // ipv6
            .read([domain_name.as_bytes(), &[10, 10]].concat().as_slice()) // domain
            .build();

        // IPv4
        assert_eq!(
            ipv4_socket_address!(Ipv4Addr::new(127, 0, 0, 1,), 2570),
            Address::read_ipv4(&mut mock).await.unwrap()
        );

        // IPv6
        assert_eq!(
            ipv6_socket_address!(Ipv6Addr::new(0, 0, 0xff, 0xffc0, 0xa02, 0xffca, 0x100, 0x11ff), 2570),
            Address::read_ipv6(&mut mock).await.unwrap()
        );

        // Domain name
        assert_eq!(
            Address::DomainName(domain_name, 2570),
            Address::read_domain_name(&mut mock, domain_name_len).await.unwrap()
        )
    }
}
