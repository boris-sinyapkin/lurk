use super::{consts, Address, AuthMethod, ReplyStatus};
use crate::io::LurkResponse;
use anyhow::Result;
use bytes::{BufMut, BytesMut};
use tokio::io::AsyncWriteExt;

// The server selects from one of the methods given in METHODS, and
// sends a METHOD selection message:
// +----+--------+
// |VER | METHOD |
// +----+--------+
// | 1  |   1    |
// +----+--------+

#[derive(Debug, PartialEq)]
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
            .map_or_else(|| consts::auth::SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE, |m| m as u8);
        let response: [u8; 2] = [consts::SOCKS5_VERSION, method];
        stream.write_all(&response).await?;
        Ok(())
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
