use clap::Parser;
use std::net::Ipv4Addr;

pub const LOG4RS_CONFIG_FILE_PATH: &str = "log4rs.yaml";

#[derive(Default, Parser, Debug)]
#[clap(author = "Boris S. <boris.works@hotmail.com>", about = "Fast and fancy SOCKS5 proxy", version)]
pub struct LurkConfig {
    /// TCP port to listen on
    #[clap(short, long, default_value_t = 1080)]
    port: u16,

    /// IPv4 to listen on
    #[clap(short, long, default_value = "0.0.0.0")]
    ipv4: Option<Ipv4Addr>,
}

impl LurkConfig {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn ipv4(&self) -> Ipv4Addr {
        self.ipv4.expect("IPv4 should have correct format")
    }
}