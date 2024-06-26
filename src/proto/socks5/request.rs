use super::{Address, Command};
use crate::{auth::LurkAuthMethod, common::error::InvalidValue, io::LurkRequest, proto::socks5::consts};
use anyhow::{ensure, Result};
use std::collections::HashSet;
use tokio::io::AsyncReadExt;

#[cfg(test)]
use tokio::io::AsyncWriteExt;

// The client connects to the server, and sends a
// version identifier/method selection message:
// +----+----------+----------+
// |VER | NMETHODS | METHODS  |
// +----+----------+----------+
// | 1  |    1     | 1 to 255 |
// +----+----------+----------+

#[derive(Debug)]
pub struct HandshakeRequest {
    auth_methods: HashSet<LurkAuthMethod>,
}

impl HandshakeRequest {
    #[cfg(test)]
    pub fn new(auth_methods: HashSet<LurkAuthMethod>) -> HandshakeRequest {
        HandshakeRequest { auth_methods }
    }

    #[cfg(test)]
    pub async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) {
        let mut packet = vec![consts::SOCKS5_VERSION, self.auth_methods.len() as u8];
        self.auth_methods.iter().for_each(|m| packet.push(m.as_socks5_const()));
        stream.write_all(&packet).await.unwrap();
    }

    pub fn auth_methods(&self) -> &HashSet<LurkAuthMethod> {
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
        ensure!(version == consts::SOCKS5_VERSION, InvalidValue::ProtocolVersion(version));

        // Parse requested auth methods.
        let auth_methods = match nmethods {
            0 => HashSet::new(),
            n => {
                let mut methods = vec![0; n.into()];
                stream.read_exact(&mut methods).await?;

                // Drop unknown auth methods.
                methods
                    .iter()
                    .map(|&m| LurkAuthMethod::from_socks5_const(m))
                    .collect::<Result<HashSet<LurkAuthMethod>>>()?
            }
        };

        Ok(HandshakeRequest { auth_methods })
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
    endpoint_address: Address,
}

impl RelayRequest {
    pub fn command(&self) -> Command {
        self.command
    }

    pub fn endpoint_address(&self) -> &Address {
        &self.endpoint_address
    }
}

impl LurkRequest for RelayRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<RelayRequest> {
        let mut buff: [u8; 3] = [0, 0, 0];
        stream.read_exact(&mut buff).await?;

        let (version, cmd, reserved) = (buff[0], buff[1], buff[2]);

        ensure!(version == consts::SOCKS5_VERSION, InvalidValue::ProtocolVersion(version));
        ensure!(reserved == 0x00, InvalidValue::ReservedValue(reserved));

        let command = Command::try_from(cmd)?;
        let endpoint_address = Address::read_from(stream).await?;

        Ok(RelayRequest { command, endpoint_address })
    }
}
