use anyhow::Result;
use tokio::io::{copy_bidirectional, AsyncRead, AsyncWrite};

pub struct LurkTunnel<'a, X, Y> {
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
