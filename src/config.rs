use clap::Parser;
use std::net::Ipv4Addr;

pub const LOG4RS_CONFIG_FILE_PATH: &str = "log4rs.yaml";

#[derive(Default, Parser, Debug)]
#[clap(author = "Boris S. <boris.works@hotmail.com>", about = "Fast and fancy SOCKS5 proxy", version)]
pub struct LurkConfig {
    /// TCP port to listen on
    #[clap(short = 'p', long, default_value_t = 1080)]
    bind_port: u16,

    /// IPv4 to listen on
    #[clap(short = 'i', long, default_value = "0.0.0.0")]
    bind_ipv4: Option<Ipv4Addr>,

    /// Limit number of simulatinously opened TCP connections
    #[clap(short = 'l', long, default_value_t = 1024)]
    tcp_conn_limit: usize,
}

impl LurkConfig {
    pub fn bind_port(&self) -> u16 {
        self.bind_port
    }

    pub fn bind_ipv4(&self) -> Ipv4Addr {
        self.bind_ipv4.expect("IPv4 should have correct format")
    }

    pub fn tcp_conn_limit(&self) -> usize {
        self.tcp_conn_limit
    }
}
