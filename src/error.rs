use crate::proto::socks5::Command;
use std::net::SocketAddr;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum LurkError {
    #[error("data has incorrect / corrupted field: {0}")]
    DataError(InvalidValue),
    #[error("failed UTF-8 decoding of domain name: {0}")]
    DomainNameDecodingFailed(std::string::FromUtf8Error),
    #[error("{0} is not supported")]
    Unsupported(Unsupported),
    #[error("unable to agree on authentication method with client {0:?}")]
    NoAcceptableAuthMethod(SocketAddr),
}

#[derive(Error, Debug, PartialEq)]
pub enum InvalidValue {
    #[error("invalid 'reserved' value {0:#02x}")]
    ReservedValue(u8),
    #[error("invalid type of network address {0:#02x}")]
    AddressType(u8),
    #[error("invalid version of protocol {0:#02x}")]
    ProtocolVersion(u8),
    #[error("invalid authenticaton method {0:#02x}")]
    AuthMethod(u8),
    #[error("invalid SOCKS command {0:#02x}")]
    SocksCommand(u8),
}

#[derive(Error, Debug, PartialEq)]
pub enum Unsupported {
    #[error("{0:?} SOCKS5 command")]
    Socks5Command(Command),
    #[error("IPv6 network address")]
    IPv6Address,
    #[error("Domain network address")]
    DomainNameAddress,
}
