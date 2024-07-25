use clap::Parser;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const LOG4RS_CONFIG_FILE_PATH: &str = "log4rs.yaml";

#[derive(Default, Parser, Debug)]
#[clap(author = "Boris S. <boris.works@hotmail.com>", about = "Fast and fancy SOCKS5 proxy", version)]
pub struct LurkConfig {
    #[command(flatten)]
    proxy_server_config: LurkProxyServerConfig,

    #[command(flatten)]
    http_endpoint_config: LurkHttpEndpointConfig,
}

#[derive(Default, Parser, Debug)]
struct LurkHttpEndpointConfig {
    /// Spin up HTTP endpoint in a background thread
    #[arg(long, default_value_t = false)]
    http_endpoint_enabled: bool,

    /// TCP port to serve HTTP requests
    #[arg(long, default_value_t = 8080)]
    http_endpoint_port: u16,
}

#[derive(Default, Parser, Debug)]
struct LurkProxyServerConfig {
    /// Proxy server TCP port to listen on
    #[arg(short = 'p', long, default_value_t = 1080)]
    proxy_port: u16,

    /// Proxy server IPv4 address to listen on
    #[arg(short = 'i', long, default_value = "0.0.0.0")]
    proxy_ipv4: Option<Ipv4Addr>,
}

impl LurkConfig {
    pub fn server_tcp_bind_addr(&self) -> SocketAddr {
        let port = self.proxy_server_config.proxy_port;
        let ipv4 = self.proxy_server_config.proxy_ipv4.expect("IPv4 should have correct format");

        SocketAddr::new(IpAddr::V4(ipv4), port)
    }

    pub fn http_endpoint_bind_addr(&self) -> Option<SocketAddr> {
        if !self.http_endpoint_config.http_endpoint_enabled {
            return None;
        }

        let ipv4 = Ipv4Addr::new(0, 0, 0, 0);
        let port = self.http_endpoint_config.http_endpoint_port;

        Some(SocketAddr::new(IpAddr::V4(ipv4), port))
    }
}
