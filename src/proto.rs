pub mod socks5 {

    use anyhow::Result;
    use log::{error, trace};
    use std::fmt::Write;
    use tokio::io::AsyncReadExt;

    ///
    /// Socks5 protocol implementation details
    ///
    /// RFC 1928
    /// https://datatracker.ietf.org/doc/html/rfc1928#ref-1
    ///

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
    #[derive(Debug)]
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

    pub struct AuthMethodRequest {
        auth_methods: Vec<AuthMethod>,
    }

    impl AuthMethodRequest {
        pub async fn from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<AuthMethodRequest> {
            let mut header: [u8; 2] = [0, 0];
            stream.read_exact(&mut header).await?;

            let (version, nmethods) = (header[0], header[1]);

            // Bail out if version is not supported.
            if version != consts::SOCKS5_VERSION {
                todo!()
            }

            // Parse requested auth methods.
            let auth_methods = match nmethods {
                0 => vec![],
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
                "version: {version:#2x}, nmethods: {nmethods:#02x}, methods: [ {}]",
                auth_methods.iter().fold(String::new(), |mut output, m| {
                    let _ = write!(output, "{m:?} ");
                    output
                })
            );

            Ok(AuthMethodRequest { auth_methods })
        }

        pub fn auth_methods(&self) -> &Vec<AuthMethod> {
            &self.auth_methods
        }
    }
}
