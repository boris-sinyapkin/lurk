use super::LurkPeer;
use crate::{
    common::LurkAuthMethod,
    io::{LurkRequestRead, LurkResponseWrite},
};
use log::error;
use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
};
use tokio::io::{AsyncRead, AsyncWrite};

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

    pub fn authenticate<S>(&self, peer: &LurkPeer<S>) -> bool
    where
        S: LurkRequestRead + LurkResponseWrite + Unpin + DerefMut,
        <S as Deref>::Target: AsyncRead + AsyncWrite + Unpin,
    {
        match self.current_method() {
            Some(method) => match method {
                LurkAuthMethod::None => true,
                _ => todo!("Unsupported authentication method {:?}", method),
            },
            None => {
                error!("Tried to authenticate {peer:}, but method has not been selected");
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
    fn pick_auth_method() {
        let peer_methods = HashSet::from([LurkAuthMethod::GssAPI, LurkAuthMethod::Password, LurkAuthMethod::None]);
        {
            let mut authenticator = LurkAuthenticator::new();
            authenticator.select_auth_method(&peer_methods);
            assert_eq!(Some(LurkAuthMethod::None), authenticator.current_method());
        }
    }
}
