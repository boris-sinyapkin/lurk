use crate::net::tcp::connection::LurkTcpConnection;
use log::error;
use std::collections::HashSet;

#[repr(u8)]
#[rustfmt::skip]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum LurkAuthMethod {
    None,
    GssAPI,
    Password,
}

pub struct LurkAuthenticator {
    available_methods: HashSet<LurkAuthMethod>,
    selected_method: Option<LurkAuthMethod>,
}

impl LurkAuthenticator {
    // Methods supported by authenticator
    const SUPPORTED_AUTH_METHODS: [LurkAuthMethod; 1] = [LurkAuthMethod::None];

    pub fn new() -> LurkAuthenticator {
        LurkAuthenticator {
            selected_method: None,
            available_methods: HashSet::from(LurkAuthenticator::SUPPORTED_AUTH_METHODS),
        }
    }

    pub fn authenticate_connection(&self, conn: &LurkTcpConnection) -> bool {
        match self.current_method() {
            Some(method) => match method {
                LurkAuthMethod::None => true,
                _ => todo!("Unsupported authentication method {:?}", method),
            },
            None => {
                error!("Tried to authenticate {}, but method has not been selected", conn.peer_addr());
                false
            }
        }
    }

    /// Find any common authentication method between available
    /// auth methods on server and supported methods by client.
    pub fn select_auth_method(&mut self, peer_methods: &HashSet<LurkAuthMethod>) -> Option<LurkAuthMethod> {
        let common_methods = self
            .available_methods
            .intersection(peer_methods)
            .collect::<HashSet<&LurkAuthMethod>>();

        self.selected_method = common_methods.into_iter().nth(0).copied();
        self.selected_method
    }

    pub fn current_method(&self) -> Option<LurkAuthMethod> {
        self.selected_method
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_auth_method() {
        let peer_methods = HashSet::from([LurkAuthMethod::GssAPI, LurkAuthMethod::Password, LurkAuthMethod::None]);
        {
            let mut authenticator = LurkAuthenticator::new();
            authenticator.select_auth_method(&peer_methods);
            assert_eq!(Some(LurkAuthMethod::None), authenticator.current_method());
        }
    }
}
