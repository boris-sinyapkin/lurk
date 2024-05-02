use super::{LurkRequest, LurkRequestRead, LurkResponse, LurkResponseWrite};
use anyhow::Result;
use log::trace;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// Alias for stream wrapper over `TcpStream`
pub type LurkTcpStream = LurkStream<TcpStream>;

/// Stream wrapper implementation

pub struct LurkStream<T> {
    stream: T,
}

impl<T> LurkStream<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    pub fn new(stream: T) -> LurkStream<T> {
        LurkStream { stream }
    }
}

impl<T> LurkRequestRead for LurkStream<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    async fn read_request<Request>(&mut self) -> Result<Request>
    where
        Request: LurkRequest + Debug,
    {
        let request = Request::read_from(&mut self.stream).await?;
        trace!("Read {:?}", request);

        Ok(request)
    }
}

impl<T> LurkResponseWrite for LurkStream<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    async fn write_response<Response>(&mut self, response: Response) -> Result<()>
    where
        Response: LurkResponse + Debug,
    {
        Response::write_to(&response, &mut self.stream).await?;
        trace!("Write {:?}", response);

        Ok(())
    }
}

impl<T> Deref for LurkStream<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<T> DerefMut for LurkStream<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}
