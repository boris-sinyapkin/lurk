use std::collections::HashSet;

use crate::{client::LurkClient, proto::socks5::AuthMethod};

pub struct LurkAuthenticator {}

impl LurkAuthenticator {
    const SUPPORTED_METHODS: [AuthMethod; 1] = [AuthMethod::None];

    #[allow(unused_variables)]
    pub fn authenticate(client: &LurkClient, method: AuthMethod) -> bool {
        match method {
            AuthMethod::None => true,
            _ => todo!(),
        }
    }

    pub fn available_methods() -> HashSet<AuthMethod> {
        HashSet::from(LurkAuthenticator::SUPPORTED_METHODS)
    }
}
