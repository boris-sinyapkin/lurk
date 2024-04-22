use crate::proto::socks5::Command;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum LurkError {
    #[error("data has incorrect / corrupted field: {0}")]
    DataError(InvalidValue),
    #[error("failed UTF-8 decoding of domain name: {0}")]
    DomainNameDecodingFailed(std::string::FromUtf8Error),
    #[error("SOCKS command {0:?} is not supported")]
    UnsupportedSocksCommand(Command),
    #[error("unable to resolve domain name {0}")]
    UnresolvedDomainName(String),
    #[error("unable to select appropriate authentication method")]
    NoAcceptableAuthMethod
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
