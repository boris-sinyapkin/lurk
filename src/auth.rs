use crate::{
    client::LurkClient,
    proto::{
        message::{LurkRequestRead, LurkResponseWrite},
        socks5::AuthMethod,
    },
};
use std::collections::HashSet;

pub struct LurkAuthenticator {
    available_methods: HashSet<AuthMethod>,
}

impl LurkAuthenticator {
    pub fn new(auth_enabled: bool) -> LurkAuthenticator {
        let available_methods = if !auth_enabled {
            HashSet::from([AuthMethod::None])
        } else {
            HashSet::from([])
        };
        LurkAuthenticator { available_methods }
    }

    #[allow(unused_variables)]
    pub fn authenticate<S: LurkRequestRead + LurkResponseWrite + Unpin>(
        &self,
        client: &LurkClient<S>,
        method: AuthMethod,
    ) -> bool {
        assert!(self.available_methods.contains(&method));
        match method {
            AuthMethod::None => true,
            _ => todo!("Unsupported authentication method {:?}", method),
        }
    }

    /// Find any common authentication method between available
    /// auth methods on server and supported methods by client.
    pub fn select_auth_method(&self, client_methods: &HashSet<AuthMethod>) -> Option<AuthMethod> {
        let common_methods = self
            .available_methods
            .intersection(client_methods)
            .collect::<HashSet<&AuthMethod>>();

        common_methods.into_iter().nth(0).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_none_auth_method() {
        let client_methods = HashSet::from([AuthMethod::GssAPI, AuthMethod::Password, AuthMethod::None]);
        assert_eq!(
            Some(AuthMethod::None),
            LurkAuthenticator::new(false).select_auth_method(&client_methods)
        );
        assert_eq!(None, LurkAuthenticator::new(true).select_auth_method(&client_methods));
    }
}
