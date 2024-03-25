use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub trait LurkRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Self>
    where
        Self: std::marker::Sized;
}

pub trait LurkResponse {
    async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()>;
}
