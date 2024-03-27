///
/// Socks5 protocol implementation details
///
/// RFC 1928
/// https://datatracker.ietf.org/doc/html/rfc1928#ref-1
///
use anyhow::{bail, ensure, Result};
use bytes::{BufMut, BytesMut};
use log::error;
use std::{
    collections::HashSet,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{InvalidValue, LurkError, Unsupported};

use self::consts::auth::SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE;

use super::message::{LurkRequest, LurkResponse};

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
        let sock = SocketAddr::V4(SocketAddrV4::new(ipv4, port));

        Ok(Address::SocketAddress(sock))
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

// The client connects to the server, and sends a
// version identifier/method selection message:
// +----+----------+----------+
// |VER | NMETHODS | METHODS  |
// +----+----------+----------+
// | 1  |    1     | 1 to 255 |
// +----+----------+----------+

#[derive(Debug)]
pub struct HandshakeRequest {
    auth_methods: HashSet<AuthMethod>,
}

impl HandshakeRequest {
    pub fn auth_methods(&self) -> &HashSet<AuthMethod> {
        &self.auth_methods
    }
}

impl LurkRequest for HandshakeRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Self>
    where
        Self: std::marker::Sized,
    {
        let mut header: [u8; 2] = [0, 0];
        stream.read_exact(&mut header).await?;

        let (version, nmethods) = (header[0], header[1]);

        // Bail out if version is not supported.
        ensure!(
            version == consts::SOCKS5_VERSION,
            InvalidValue::ProtocolVersion(version)
        );

        // Parse requested auth methods.
        let auth_methods = match nmethods {
            0 => HashSet::new(),
            n => {
                let mut methods = vec![0; n.into()];
                stream.read_exact(&mut methods).await?;

                // Drop unknown auth methods.
                methods
                    .iter()
                    .filter_map(|&m| match AuthMethod::try_from(m) {
                        Ok(auth_method) => Some(auth_method),
                        Err(err) => {
                            error!("{}", err);
                            None
                        }
                    })
                    .collect()
            }
        };

        Ok(HandshakeRequest { auth_methods })
    }
}

// The server selects from one of the methods given in METHODS, and
// sends a METHOD selection message:
// +----+--------+
// |VER | METHOD |
// +----+--------+
// | 1  |   1    |
// +----+--------+

#[derive(Debug)]
pub struct HandshakeResponse {
    selected_method: Option<AuthMethod>,
}

impl HandshakeResponse {
    pub fn new(selected_method: Option<AuthMethod>) -> HandshakeResponse {
        HandshakeResponse { selected_method }
    }
}

impl LurkResponse for HandshakeResponse {
    async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()> {
        let method = self
            .selected_method
            .map_or_else(|| SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE, |m| m as u8);
        let response: [u8; 2] = [consts::SOCKS5_VERSION, method];
        stream.write_all(&response).await?;
        Ok(())
    }
}

// The SOCKS request information is sent by the client as
// soon as it has established a connection to the SOCKS
// server, and completed the authentication negotiations.
// +----+-----+-------+------+----------+----------+
// |VER | CMD |  RSV  | ATYP | DST.ADDR | DST.PORT |
// +----+-----+-------+------+----------+----------+
// | 1  |  1  | X'00' |  1   | Variable |    2     |
// +----+-----+-------+------+----------+----------+

#[derive(Debug)]
pub struct RelayRequest {
    command: Command,
    target_addr: Address,
}

impl RelayRequest {
    pub fn command(&self) -> Command {
        self.command
    }

    pub fn target_addr(&self) -> &Address {
        &self.target_addr
    }
}

impl LurkRequest for RelayRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<RelayRequest> {
        let mut buff: [u8; 3] = [0, 0, 0];
        stream.read_exact(&mut buff).await?;

        let (version, cmd, reserved) = (buff[0], buff[1], buff[2]);

        ensure!(
            version == consts::SOCKS5_VERSION,
            InvalidValue::ProtocolVersion(version)
        );
        ensure!(reserved == 0x00, InvalidValue::ReservedValue(reserved));

        let command = Command::try_from(cmd)?;
        let target_addr = Address::read_from(stream).await?;

        Ok(RelayRequest { command, target_addr })
    }
}

#[derive(Debug, Clone, Copy)]
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
                Unsupported::IPv6Address | Unsupported::DomainNameAddress => ReplyStatus::AddressTypeNotSupported,
            },
            LurkError::NoAcceptableAuthMethod(_) => ReplyStatus::ConnectionNotAllowed,
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

// The server evaluates the relay request, and returns a reply formed as follows:
// +----+-----+-------+------+----------+----------+
// |VER | REP |  RSV  | ATYP | BND.ADDR | BND.PORT |
// +----+-----+-------+------+----------+----------+
// | 1  |  1  | X'00' |  1   | Variable |    2     |
// +----+-----+-------+------+----------+----------+

#[derive(Debug)]
pub struct RelayResponse {
    bound_addr: Address,
    status: ReplyStatus,
}

impl RelayResponse {
    pub fn new(bound_addr: Address, status: ReplyStatus) -> RelayResponse {
        RelayResponse { bound_addr, status }
    }
}

impl LurkResponse for RelayResponse {
    async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()> {
        let mut bytes = BytesMut::new();
        bytes.put_slice(&[consts::SOCKS5_VERSION, self.status.as_u8(), 0x00]);
        self.bound_addr.write_to(&mut bytes);
        stream.write_all(&bytes).await?;
        Ok(())
    }
}
