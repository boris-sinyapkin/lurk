use super::{consts, Address, ReplyStatus};
use crate::{common::LurkAuthMethod, io::LurkResponse};
use anyhow::{bail, Result};
use bytes::{BufMut, BytesMut};
use log::error;
use std::net::SocketAddr;
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
    method: u8,
}

impl HandshakeResponse {
    pub fn builder() -> HandshakeResponseBuilder {
        HandshakeResponseBuilder { method: None }
    }
}

impl LurkResponse for HandshakeResponse {
    async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()> {
        use consts::auth::*;
        // Just checking that method value is benign.
        if self.method != SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE {
            if let Err(err) = LurkAuthMethod::from_socks5_const(self.method) {
                error!("Unable to convert authentication method to any of SOCKS5 constants");
                debug_assert!(false);
                bail!(err)
            }
        }
        let response: [u8; 2] = [consts::SOCKS5_VERSION, self.method];
        stream.write_all(&response).await?;
        Ok(())
    }
}

pub struct HandshakeResponseBuilder {
    method: Option<u8>,
}

impl HandshakeResponseBuilder {
    pub fn with_auth_method(&mut self, selected_method: LurkAuthMethod) -> &mut HandshakeResponseBuilder {
        debug_assert!(self.method.is_none(), "should be unset");
        self.method = Some(selected_method as u8);
        self
    }

    pub fn with_no_acceptable_method(&mut self) -> &mut HandshakeResponseBuilder {
        debug_assert!(self.method.is_none(), "should be unset");
        self.method = Some(consts::auth::SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE);
        self
    }

    pub fn build(&self) -> HandshakeResponse {
        HandshakeResponse {
            method: self.method.expect("Expected valid SOCKS5 authentication constant"),
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
    pub fn builder() -> RelayResponseBuilder {
        RelayResponseBuilder {
            bound_addr: None,
            status: None,
        }
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

pub struct RelayResponseBuilder {
    bound_addr: Option<Address>,
    status: Option<ReplyStatus>,
}

impl RelayResponseBuilder {
    pub fn with_success(&mut self) -> &mut RelayResponseBuilder {
        debug_assert!(self.status.is_none(), "should be unset");
        self.status = Some(ReplyStatus::Succeeded);
        self
    }

    pub fn with_err(&mut self, err: anyhow::Error) -> &mut RelayResponseBuilder {
        debug_assert!(self.status.is_none(), "should be unset");
        self.status = Some(ReplyStatus::from(err));
        self
    }

    pub fn with_bound_address(&mut self, bound_addr: SocketAddr) -> &mut RelayResponseBuilder {
        debug_assert!(self.bound_addr.is_none(), "should be unset");
        self.bound_addr = Some(Address::SocketAddress(bound_addr));
        self
    }

    pub fn build(&self) -> RelayResponse {
        RelayResponse {
            bound_addr: self.bound_addr.clone().expect("Bound address expected"),
            status: self.status.expect("Reply status expected"),
        }
    }
}
