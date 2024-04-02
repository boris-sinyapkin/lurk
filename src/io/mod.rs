use anyhow::Result;
use std::fmt::Debug;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod stream;

pub trait LurkRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Self>
    where
        Self: std::marker::Sized;
}

pub trait LurkResponse {
    async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()>;
}

pub trait LurkResponseWrite {
    async fn write_response<Response>(&mut self, response: Response) -> Result<()>
    where
        Response: LurkResponse + Debug + 'static;
}

pub trait LurkRequestRead {
    async fn read_request<Request>(&mut self) -> Result<Request>
    where
        Request: LurkRequest + Debug + 'static;
}
