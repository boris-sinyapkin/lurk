use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use anyhow::Result;
use log::trace;
use mockall::mock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Traits

pub trait LurkRequest {
    async fn read_from<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Self>
    where
        Self: std::marker::Sized;
}

pub trait LurkResponse {
    async fn write_to<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()>;
}

pub trait LurkResponseWriter {
    async fn write_response<Response>(&mut self, response: Response) -> Result<()>
    where
        Response: LurkResponse + Debug + 'static;
}

pub trait LurkRequestReader {
    async fn read_request<Request>(&mut self) -> Result<Request>
    where
        Request: LurkRequest + Debug + 'static;
}

/// Stream implementation

pub struct LurkStreamWrapper<Stream>
where
    Stream: AsyncReadExt + AsyncWriteExt + Unpin,
{
    stream: Stream,
}

impl<Stream> LurkStreamWrapper<Stream>
where
    Stream: AsyncReadExt + AsyncWriteExt + Unpin,
{
    pub fn new(stream: Stream) -> LurkStreamWrapper<Stream> {
        LurkStreamWrapper { stream }
    }
}

impl<Stream> LurkRequestReader for LurkStreamWrapper<Stream>
where
    Stream: AsyncReadExt + AsyncWriteExt + Unpin,
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

impl<Stream> LurkResponseWriter for LurkStreamWrapper<Stream>
where
    Stream: AsyncReadExt + AsyncWriteExt + Unpin,
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

impl<Stream> Deref for LurkStreamWrapper<Stream>
where
    Stream: AsyncReadExt + AsyncWriteExt + Unpin,
{
    type Target = Stream;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl<Stream> DerefMut for LurkStreamWrapper<Stream>
where
    Stream: AsyncReadExt + AsyncWriteExt + Unpin,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

mock! {
    pub LurkStreamWrapper<Stream: AsyncReadExt + AsyncWriteExt + Unpin + 'static> {}

    impl<Stream: AsyncReadExt + AsyncWriteExt + Unpin> LurkRequestReader for LurkStreamWrapper<Stream> {
        async fn read_request<Request: LurkRequest + Debug + 'static>(&mut self) -> Result<Request>;
    }

    impl<Stream: AsyncReadExt + AsyncWriteExt + Unpin> LurkResponseWriter for LurkStreamWrapper<Stream> {
        async fn write_response<Response: LurkResponse + Debug + 'static>(&mut self, response: Response) -> Result<()>;
    }

    impl<Stream: AsyncReadExt + AsyncWriteExt + Unpin> Deref for LurkStreamWrapper<Stream> {
        type Target = Stream;
        fn deref(&self) -> &<MockLurkStreamWrapper<Stream> as Deref>::Target;
    }

    impl<Stream: AsyncReadExt + AsyncWriteExt + Unpin> DerefMut for LurkStreamWrapper<Stream> {
        fn deref_mut(&mut self) -> &mut Stream;
    }
}