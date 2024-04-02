///
/// Socks5 protocol implementation details
///
/// RFC 1928
/// https://datatracker.ietf.org/doc/html/rfc1928#ref-1
///
use crate::error::{InvalidValue, LurkError, Unsupported};
use anyhow::{bail, Result};
use bytes::BufMut;
use std::{
    fmt::Display,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};
use tokio::io::AsyncReadExt;

pub mod request;
pub mod response;

#[cfg(test)]
mod test;

macro_rules! ipv4_socket_address {
    ($ipv4:expr, $port:expr) => {
        Address::SocketAddress(SocketAddr::V4(SocketAddrV4::new($ipv4, $port)))
    };
}

pub(crate) use ipv4_socket_address;

#[rustfmt::skip]
mod consts {
    pub const SOCKS5_VERSION: u8 = 0x05;

    pub mod auth {
        pub const SOCKS5_AUTH_METHOD_NONE: u8 = 0x00;
        pub const SOCKS5_AUTH_METHOD_GSSAPI: u8 = 0x01;
        pub const SOCKS5_AUTH_METHOD_PASSWORD: u8 = 0x02;
        pub const SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE: u8 = 0xff;
    }

    pub mod command {
        pub const SOCKS5_CMD_CONNECT: u8 = 0x01;
        pub const SOCKS5_CMD_BIND: u8 = 0x02;
        pub const SOCKS5_CMD_UDP_ASSOCIATE: u8 = 0x03;
    }

    pub mod address {
        pub const SOCKS5_ADDR_TYPE_IPV4: u8 = 0x01;
        pub const SOCKS5_ADDR_TYPE_DOMAIN_NAME: u8 = 0x03;
        pub const SOCKS5_ADDR_TYPE_IPV6: u8 = 0x04;
    }

    pub mod reply {
        pub const SOCKS5_REPLY_SUCCEEDED: u8 = 0x00;
        pub const SOCKS5_REPLY_GENERAL_FAILURE: u8 = 0x01;
        pub const SOCKS5_REPLY_CONNECTION_NOT_ALLOWED: u8 = 0x02;
        pub const SOCKS5_REPLY_NETWORK_UNREACHABLE: u8 = 0x03;
        pub const SOCKS5_REPLY_HOST_UNREACHABLE: u8 = 0x04;
        pub const SOCKS5_REPLY_CONNECTION_REFUSED: u8 = 0x05;
        pub const SOCKS5_REPLY_TTL_EXPIRED: u8 = 0x06;
        pub const SOCKS5_REPLY_COMMAND_NOT_SUPPORTED: u8 = 0x07;
        pub const SOCKS5_REPLY_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;
    }
}

#[repr(u8)]
#[rustfmt::skip]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum AuthMethod {
    None,
    GssAPI,
    Password,
}

impl TryFrom<u8> for AuthMethod {
    type Error = LurkError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use consts::auth::*;
        match value {
            SOCKS5_AUTH_METHOD_NONE => Ok(AuthMethod::None),
            SOCKS5_AUTH_METHOD_GSSAPI => Ok(AuthMethod::GssAPI),
            SOCKS5_AUTH_METHOD_PASSWORD => Ok(AuthMethod::Password),
            _ => Err(LurkError::DataError(InvalidValue::AuthMethod(value))),
        }
    }
}

#[repr(u8)]
#[rustfmt::skip]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Command {
    Connect,
    Bind,
    UdpAssociate
}

impl TryFrom<u8> for Command {
    type Error = LurkError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use consts::command::*;
        match value {
            SOCKS5_CMD_BIND => Ok(Command::Bind),
            SOCKS5_CMD_CONNECT => Ok(Command::Connect),
            SOCKS5_CMD_UDP_ASSOCIATE => Ok(Command::UdpAssociate),
            _ => Err(LurkError::DataError(InvalidValue::SocksCommand(value))),
        }
    }
}

#[repr(u8)]
#[rustfmt::skip]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Address {
    SocketAddress(SocketAddr),
    DomainName(String, u16)
}

impl Address {
    pub async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        use consts::address::*;
        let address_type = stream.read_u8().await?;

        match address_type {
            SOCKS5_ADDR_TYPE_IPV4 => Address::read_ipv4(stream).await,
            SOCKS5_ADDR_TYPE_IPV6 => Address::read_ipv6(stream).await,
            SOCKS5_ADDR_TYPE_DOMAIN_NAME => Address::read_domain_name(stream).await,
            _ => bail!(LurkError::DataError(InvalidValue::AddressType(address_type))),
        }
    }

    pub fn write_to<T: BufMut>(&self, buf: &mut T) {
        match self {
            Address::SocketAddress(SocketAddr::V4(ipv4_addr)) => Address::write_ipv4(buf, ipv4_addr),
            Address::SocketAddress(SocketAddr::V6(ipv6_addr)) => Address::write_ipv6(buf, ipv6_addr),
            Address::DomainName(name, port) => Address::write_domain_name(buf, name, port),
        }
    }

    fn write_ipv4<T: BufMut>(bytes: &mut T, ipv4_addr: &SocketAddrV4) {
        bytes.put_u8(consts::address::SOCKS5_ADDR_TYPE_IPV4);
        bytes.put_slice(&ipv4_addr.ip().octets());
        bytes.put_u16(ipv4_addr.port());
    }

    #[allow(unused_variables)]
    fn write_ipv6<T: BufMut>(bytes: &mut T, ipv6_addr: &SocketAddrV6) {
        todo!("Writing of IPv6 is not implemented")
    }

    #[allow(unused_variables)]
    fn write_domain_name<T: BufMut>(bytes: &mut T, name: &str, port: &u16) {
        todo!("Writing of domain names is not implemented")
    }

    async fn read_ipv4<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        let ipv4 = Ipv4Addr::from(stream.read_u32().await?);
        let port = stream.read_u16().await?;

        Ok(ipv4_socket_address!(ipv4, port))
    }

    #[allow(unused_variables)]
    async fn read_ipv6<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        bail!(LurkError::Unsupported(Unsupported::IPv6Address))
    }

    async fn read_domain_name<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        let len = stream.read_u8().await?;
        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).await?;

        let name = String::from_utf8(buf).map_err(LurkError::DomainNameDecodingFailed)?;
        let port = stream.read_u16().await?;

        Ok(Address::DomainName(name, port))
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum ReplyStatus {
    Succeeded,
    GeneralFailure,
    ConnectionNotAllowed,
    NetworkUnreachable,
    HostUnreachable,
    ConnectionRefused,
    TtlExpired,
    CommandNotSupported,
    AddressTypeNotSupported,
    OtherReply(u8),
}

impl ReplyStatus {
    #[rustfmt::skip]
    fn as_u8(self) -> u8 {
        match self {
            ReplyStatus::Succeeded               => consts::reply::SOCKS5_REPLY_SUCCEEDED,
            ReplyStatus::GeneralFailure          => consts::reply::SOCKS5_REPLY_GENERAL_FAILURE,
            ReplyStatus::ConnectionNotAllowed    => consts::reply::SOCKS5_REPLY_CONNECTION_NOT_ALLOWED,
            ReplyStatus::NetworkUnreachable      => consts::reply::SOCKS5_REPLY_NETWORK_UNREACHABLE,
            ReplyStatus::HostUnreachable         => consts::reply::SOCKS5_REPLY_HOST_UNREACHABLE,
            ReplyStatus::ConnectionRefused       => consts::reply::SOCKS5_REPLY_CONNECTION_REFUSED,
            ReplyStatus::TtlExpired              => consts::reply::SOCKS5_REPLY_TTL_EXPIRED,
            ReplyStatus::CommandNotSupported     => consts::reply::SOCKS5_REPLY_COMMAND_NOT_SUPPORTED,
            ReplyStatus::AddressTypeNotSupported => consts::reply::SOCKS5_REPLY_ADDRESS_TYPE_NOT_SUPPORTED,
            ReplyStatus::OtherReply(other)       => other,
        }
    }
}

impl From<LurkError> for ReplyStatus {
    fn from(err: LurkError) -> Self {
        match err {
            LurkError::Unsupported(unsupported) => match unsupported {
                Unsupported::Socks5Command(_) => ReplyStatus::CommandNotSupported,
                Unsupported::IPv6Address => ReplyStatus::AddressTypeNotSupported,
            },
            LurkError::NoAcceptableAuthMethod(_) => ReplyStatus::ConnectionNotAllowed,
            LurkError::UnresolvedDomainName(_) => ReplyStatus::HostUnreachable,
            _ => ReplyStatus::GeneralFailure,
        }
    }
}

impl From<anyhow::Error> for ReplyStatus {
    fn from(err: anyhow::Error) -> Self {
        let err = match err.downcast::<LurkError>() {
            Ok(lurk_proto) => return ReplyStatus::from(lurk_proto),
            Err(err) => err,
        };
        match err.downcast::<std::io::Error>() {
            Ok(io) => match io.kind() {
                std::io::ErrorKind::ConnectionRefused => ReplyStatus::ConnectionRefused,
                std::io::ErrorKind::ConnectionAborted => ReplyStatus::HostUnreachable,
                _ => ReplyStatus::GeneralFailure,
            },
            Err(_) => ReplyStatus::GeneralFailure,
        }
    }
}
