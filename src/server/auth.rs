use super::peer::LurkPeer;
use crate::{
    common::LurkAuthMethod,
    io::{LurkRequestRead, LurkResponseWrite},
};
use log::error;
use std::collections::HashSet;

pub struct LurkAuthenticator {
    available_methods: HashSet<LurkAuthMethod>,
    selected_method: Option<LurkAuthMethod>,
}

impl LurkAuthenticator {
    pub fn new(auth_enabled: bool) -> LurkAuthenticator {
        let available_methods = if !auth_enabled {
            HashSet::from([LurkAuthMethod::None])
        } else {
            HashSet::from([])
        };
        LurkAuthenticator {
            available_methods,
            selected_method: None,
        }
    }

    #[allow(unused_variables)]
    pub fn authenticate<S: LurkRequestRead + LurkResponseWrite + Unpin>(&self, peer: &LurkPeer<S>) -> bool {
        match self.current_method() {
            Some(method) => match method {
                LurkAuthMethod::None => true,
                _ => todo!("Unsupported authentication method {:?}", method),
            },
            None => {
                error!("Authentication method has not been selected");
                false
            }
        }
    }

    /// Find any common authentication method between available
    /// auth methods on server and supported methods by client.
    pub fn select_auth_method(&mut self, peer_methods: &HashSet<LurkAuthMethod>) {
        let common_methods = self
            .available_methods
            .intersection(peer_methods)
            .collect::<HashSet<&LurkAuthMethod>>();

        self.selected_method = common_methods.into_iter().nth(0).copied();
    }

    pub fn current_method(&self) -> Option<LurkAuthMethod> {
        self.selected_method
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_none_auth_method() {
        let peer_methods = HashSet::from([LurkAuthMethod::GssAPI, LurkAuthMethod::Password, LurkAuthMethod::None]);
        {
            let mut authenticator = LurkAuthenticator::new(false);
            authenticator.select_auth_method(&peer_methods);
            assert_eq!(Some(LurkAuthMethod::None), authenticator.current_method());
        }
        {
            let mut authenticator = LurkAuthenticator::new(true);
            authenticator.select_auth_method(&peer_methods);
            assert_eq!(None, authenticator.current_method());
        }
    }
}