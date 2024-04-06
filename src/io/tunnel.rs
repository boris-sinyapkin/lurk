use anyhow::{bail, Result};
use log::{debug, error};
use tokio::io::{copy_bidirectional, AsyncRead, AsyncWrite};

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

    pub async fn run(&mut self) -> Result<()> {
        match copy_bidirectional(self.l2r, self.r2l).await {
            Ok((l2r, r2l)) => debug!("Tunnel closed, L2R {} bytes, R2L {} bytes transmitted", l2r, r2l),
            Err(err) => {
                error!("Tunnel closed with error: {}", err);
                bail!(err)
            }
        }
        Ok(())
    }
}
