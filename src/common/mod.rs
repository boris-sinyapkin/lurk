pub mod error;
pub mod net;

#[repr(u8)]
#[rustfmt::skip]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum LurkAuthMethod {
    None,
    GssAPI,
    Password,
}
