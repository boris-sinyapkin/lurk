///
/// Socks5 protocol implementation details
///
/// RFC 1928
/// https://datatracker.ietf.org/doc/html/rfc1928#ref-1
///
use anyhow::Result;
use core::fmt;
use log::error;
use std::{
    collections::HashSet,
    fmt::Write,
    io::Cursor,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::message::{LurkRequest, LurkResponse};

macro_rules! expect_field_or_fail {
    ($actual: expr, $expected: expr) => {
        if $actual != $expected {
            todo!()
        }
    };
}

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
    NoAcceptableMethod
}

impl TryFrom<u8> for AuthMethod {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        use consts::auth::*;
        match value {
            SOCKS5_AUTH_METHOD_NONE => Ok(AuthMethod::None),
            SOCKS5_AUTH_METHOD_GSSAPI => Ok(AuthMethod::GssAPI),
            SOCKS5_AUTH_METHOD_PASSWORD => Ok(AuthMethod::Password),
            SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE => Ok(AuthMethod::NoAcceptableMethod),
            _ => Err(Self::Error::msg(format!(
                "Failed to convert the value {:#02x} to any of SOCKS5 auth. constants",
                value
            ))),
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
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        use consts::command::*;
        match value {
            SOCKS5_CMD_BIND => Ok(Command::Bind),
            SOCKS5_CMD_CONNECT => Ok(Command::Connect),
            SOCKS5_CMD_UDP_ASSOCIATE => Ok(Command::UdpAssociate),
            _ => Err(Self::Error::msg(format!(
                "Failed to convert the value {:#02x} to any of SOCKS5 command constants",
                value
            ))),
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
    pub async fn parse_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        use consts::address::*;
        let address_type = stream.read_u8().await?;

        match address_type {
            SOCKS5_ADDR_TYPE_IPV4 => Address::parse_ipv4(stream).await,
            SOCKS5_ADDR_TYPE_IPV6 => todo!(),
            SOCKS5_ADDR_TYPE_DOMAIN_NAME => Address::parse_domain_name(stream).await,
            _ => todo!(),
        }
    }

    async fn parse_ipv4<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        let ipv4 = Ipv4Addr::from(stream.read_u32().await?);
        let port = stream.read_u16().await?;
        let sock = SocketAddr::V4(SocketAddrV4::new(ipv4, port));

        Ok(Address::SocketAddress(sock))
    }

    fn parse_ipv6<T: AsRef<[u8]>>(cursor: &mut Cursor<T>) -> Result<Address> {
        todo!()
    }

    async fn parse_domain_name<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Address> {
        let len = stream.read_u8().await?;
        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).await?;

        let name = String::from_utf8(buf)?;
        let port = stream.read_u16().await?;

        Ok(Address::DomainName(name, port))
    }
}

pub enum Reply {
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

// The client connects to the server, and sends a
// version identifier/method selection message:
// +----+----------+----------+
// |VER | NMETHODS | METHODS  |
// +----+----------+----------+
// | 1  |    1     | 1 to 255 |
// +----+----------+----------+

pub struct HandshakeRequest {
    auth_methods: HashSet<AuthMethod>,
}

impl HandshakeRequest {
    pub fn auth_methods(&self) -> &HashSet<AuthMethod> {
        &self.auth_methods
    }
}

impl LurkRequest for HandshakeRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Self> where Self: std::marker::Sized {
        let mut header: [u8; 2] = [0, 0];
        stream.read_exact(&mut header).await?;

        let (version, nmethods) = (header[0], header[1]);

        // Bail out if version is not supported.
        expect_field_or_fail!(version, consts::SOCKS5_VERSION);

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

impl std::fmt::Display for HandshakeRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HandshakeRequest [ auth_methods: [ {}] ]",
            self.auth_methods.iter().fold(String::new(), |mut output, m| {
                let _ = write!(output, "{m:?} ");
                output
            })
        )
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
    selected_method: AuthMethod,
}

impl HandshakeResponse {
    pub fn new(selected_method: AuthMethod) -> HandshakeResponse {
        HandshakeResponse { selected_method }
    }
}

impl LurkResponse for HandshakeResponse {
    async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()> {
        let response: [u8; 2] = [consts::SOCKS5_VERSION, self.selected_method as u8];
        Ok(stream.write_all(&response).await?)
    }
}

impl std::fmt::Display for HandshakeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HandshakeResponse [ selected_method: {:?} ]", self.selected_method)
    }
}

// Once the method-dependent subnegotiation has completed, the client
// sends the request details.
// +----+-----+-------+------+----------+----------+
// |VER | CMD |  RSV  | ATYP | DST.ADDR | DST.PORT |
// +----+-----+-------+------+----------+----------+
// | 1  |  1  | X'00' |  1   | Variable |    2     |
// +----+-----+-------+------+----------+----------+

pub struct RelayRequest {
    command: Command,
    dst_addr: Address,
}

impl LurkRequest for RelayRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<RelayRequest> {
        let mut buff: [u8; 3] = [0, 0, 0];
        stream.read_exact(&mut buff).await?;

        let (version, cmd, reserved) = (buff[0], buff[1], buff[2]);

        expect_field_or_fail!(version, consts::SOCKS5_VERSION);
        expect_field_or_fail!(reserved, 0x00);

        let command = Command::try_from(cmd)?;
        let dst_addr = Address::parse_from(stream).await?;

        Ok(RelayRequest { command, dst_addr })
    }
}

impl std::fmt::Display for RelayRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RelayRequest [ command: {:?}, dst_address: {:?} ]",
            self.command, self.dst_addr
        )
    }
}

// The SOCKS request information is sent by the client as soon as it has
// established a connection to the SOCKS server, and completed the
// authentication negotiations.  The server evaluates the request, and
// returns a reply formed as follows:

// +----+-----+-------+------+----------+----------+
// |VER | REP |  RSV  | ATYP | BND.ADDR | BND.PORT |
// +----+-----+-------+------+----------+----------+
// | 1  |  1  | X'00' |  1   | Variable |    2     |
// +----+-----+-------+------+----------+----------+

pub struct RelayResponse {
    bound_addr: Address,
    bound_port: u16,
}

