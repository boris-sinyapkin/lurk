use thiserror::Error;

pub mod message;
pub mod socks5;

#[derive(Error, Debug)]
pub enum InvalidProtoField {
    #[error("reserved field has invalid value {0:#02x}")]
    ReservedValue(u8),
    #[error("invalid type of network address {0:#02x}")]
    AddressType(u8),
    #[error("invalid version of protocol {0:#02x}")]
    ProtocolVersion(u8),
    #[error("invalid authenticaton method {0:#02x}")]
    AuthMethod(u8),
    #[error("invalid SOCKS command {0:#02x}")]
    SocksCommand(u8)
}