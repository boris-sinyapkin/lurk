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

#[cfg(test)]
use mockall::mock;

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

#[cfg(test)]
mock! {
  pub LurkStreamWrapper<T: AsyncReadExt + AsyncWriteExt + Unpin + 'static> {}

  impl<T: AsyncReadExt + AsyncWriteExt + Unpin> LurkRequestRead for LurkStreamWrapper<T> {
      async fn read_request<Request: LurkRequest + Debug + 'static>(&mut self) -> Result<Request>;
  }

  impl<T: AsyncReadExt + AsyncWriteExt + Unpin> LurkResponseWrite for LurkStreamWrapper<T> {
      async fn write_response<Response: LurkResponse + Debug + 'static>(&mut self, response: Response) -> Result<()>;
  }

  impl<T: AsyncReadExt + AsyncWriteExt + Unpin> Deref for LurkStreamWrapper<T> {
      type Target = T;
      fn deref(&self) -> &<MockLurkStreamWrapper<T> as Deref>::Target;
  }

  impl<T: AsyncReadExt + AsyncWriteExt + Unpin> DerefMut for LurkStreamWrapper<T> {
      fn deref_mut(&mut self) -> &mut T;
  }
}
