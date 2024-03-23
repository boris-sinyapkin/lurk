pub mod socks5 {
    ///
    /// Socks5 protocol implementation details
    ///
    /// RFC 1928
    /// https://datatracker.ietf.org/doc/html/rfc1928#ref-1
    ///
    use anyhow::Result;
    use log::{error, trace};
    use std::{collections::HashSet, fmt::Write};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[rustfmt::skip]
    mod consts {
        pub const SOCKS5_VERSION: u8 = 0x05;

        pub mod auth {
            pub const SOCKS5_AUTH_METHOD_NONE: u8 = 0x00;
            pub const SOCKS5_AUTH_METHOD_GSSAPI: u8 = 0x01;
            pub const SOCKS5_AUTH_METHOD_PASSWORD: u8 = 0x02;
            pub const SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE: u8 = 0xff;
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
        pub async fn parse_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<HandshakeRequest> {
            let mut header: [u8; 2] = [0, 0];
            stream.read_exact(&mut header).await?;

            let (version, nmethods) = (header[0], header[1]);

            // Bail out if version is not supported.
            if version != consts::SOCKS5_VERSION {
                todo!()
            }

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

            trace!(
                "handshake request: version: {version:#2x}, nmethods: {nmethods:#02x}, methods: [ {}]",
                auth_methods.iter().fold(String::new(), |mut output, m| {
                    let _ = write!(output, "{m:?} ");
                    output
                })
            );

            Ok(HandshakeRequest { auth_methods })
        }

        pub fn auth_methods(&self) -> &HashSet<AuthMethod> {
            &self.auth_methods
        }
    }

    // The server selects from one of the methods given in METHODS, and
    // sends a METHOD selection message:
    // +----+--------+
    // |VER | METHOD |
    // +----+--------+
    // | 1  |   1    |
    // +----+--------+

    pub struct HandshakeResponse {
        selected_method: AuthMethod,
    }

    impl HandshakeResponse {
        pub fn new(selected_method: AuthMethod) -> HandshakeResponse {
            HandshakeResponse { selected_method }
        }

        pub async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()> {
            let response: [u8; 2] = [consts::SOCKS5_VERSION, self.selected_method as u8];
            Ok(stream.write_all(&response).await?)
        }
    }
}
