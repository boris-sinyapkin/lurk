use anyhow::Result;
use tokio::io::{copy_bidirectional, AsyncRead, AsyncWrite};

macro_rules! log_tunnel_created {
    ($peer:expr, $proxy:expr, $endpoint:expr) => {
        info!(
            "\n\n\tTunnel has been CREATED: \
            \n\t\tsource [{}] <--L--> lurk [{}] <--R--> destination [{}]\n",
            $peer, $proxy, $endpoint
        );
    };
}

macro_rules! log_tunnel_closed {
    ($peer:expr, $proxy:expr, $endpoint:expr, $l2r:expr, $r2l:expr) => {
        info!(
            "\n\n\tTunnel has been CLOSED: \
            \n\t\tsource [{}] <--L--> lurk [{}] <--R--> destination [{}] \
            \n\t\ttransmitted: L->R {}, R->L {}\n",
            $peer, $proxy, $endpoint, human_bytes($l2r as f64), human_bytes($r2l as f64)
        );
    };
}

macro_rules! log_tunnel_closed_with_error {
    ($peer:expr, $proxy:expr, $endpoint:expr, $err:expr) => {
        error!(
            "\n\n\tTunnel has been CLOSED with ERROR: \
            \n\t\tsource [{}] <--L--> lurk [{}] <--R--> destination [{}] \
            \n\t\terror: '{}'\n",
            $peer, $proxy, $endpoint, $err
        );
    };
}

pub(crate) use log_tunnel_created;
pub(crate) use log_tunnel_closed;
pub(crate) use log_tunnel_closed_with_error;

pub struct LurkTunnel<'a, X, Y>
where
    X: AsyncRead + AsyncWrite + Unpin,
    Y: AsyncRead + AsyncWrite + Unpin,
{
    l2r: &'a mut X,
    r2l: &'a mut Y,
}

impl<'a, X, Y> LurkTunnel<'a, X, Y>
where
    X: AsyncRead + AsyncWrite + Unpin,
    Y: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(l2r: &'a mut X, r2l: &'a mut Y) -> LurkTunnel<'a, X, Y> {
        LurkTunnel { l2r, r2l }
    }

    pub async fn run(&mut self) -> Result<(u64, u64)> {
        copy_bidirectional(self.l2r, self.r2l).await.map_err(anyhow::Error::from)
    }
}
