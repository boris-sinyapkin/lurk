use std::fmt::Debug;

use anyhow::Result;
use log::trace;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub trait LurkRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Self>
    where
        Self: std::marker::Sized;
}

pub trait LurkResponse {
    async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()>;
}

pub struct LurkMessageHandler {}

impl LurkMessageHandler {
    pub async fn read_request<Input, Request>(input: &mut Input) -> Result<Request>
    where
        Input: AsyncReadExt + Unpin,
        Request: LurkRequest + Debug,
    {
        let request = Request::read_from(input).await?;
        trace!("Read {:?}", request);

        Ok(request)
    }

    pub async fn write_response<Output, Response>(output: &mut Output, response: Response) -> Result<()>
    where
        Output: AsyncWriteExt + Unpin,
        Response: LurkResponse + Debug,
    {
        Response::write_to(&response, output).await?;
        trace!("Write {:?}", response);

        Ok(())
    }
}
