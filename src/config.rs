use clap::Parser;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const LOG4RS_CONFIG_FILE_PATH: &str = "log4rs.yaml";

#[derive(Default, Parser, Debug)]
#[clap(author = "Boris S. <boris.works@hotmail.com>", about = "Fast and fancy SOCKS5 proxy", version)]
pub struct LurkConfig {
    /// Proxy TCP port to listen on
    #[clap(short = 'p', long, default_value_t = 1080)]
    proxy_port: u16,

    /// Proxy IPv4 address to listen on
    #[clap(short = 'i', long, default_value = "0.0.0.0")]
    proxy_ipv4: Option<Ipv4Addr>,

    /// Spin up HTTP endpoint in a background thread
    #[clap(short = 'e', long, default_value_t = false)]
    enable_http_endpoint: bool
}

impl LurkConfig {
    pub fn server_tcp_bind_addr(&self) -> SocketAddr {
        SocketAddr::new(
            IpAddr::V4(self.proxy_ipv4.expect("IPv4 should have correct format")),
            self.proxy_port,
        )
    }

    pub fn enable_http_endpoint(&self) -> bool {
        self.enable_http_endpoint
    }
}
