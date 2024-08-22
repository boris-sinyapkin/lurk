use anyhow::Result;
use log::debug;
use lurk::{api::LurkHttpEndpoint, server::LurkServer};
use std::{future::Future, net::SocketAddr, sync::Arc};
use tokio::task::{yield_now, JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;

pub mod tcp_echo_server;

#[allow(unused_macros)]
macro_rules! cancel_listener {
    ($l:expr) => {
        $l.cancel().await.expect("Failed to cancel async task");
    };
}

#[allow(unused_imports)]
pub(crate) use cancel_listener;

pub trait AsyncListener {
    fn name(&self) -> &'static str;

    fn listen(&mut self) -> impl Future<Output = Result<()>> + Send;

    fn run(self) -> impl Future<Output = AsyncListenerTask> + Send
    where
        Self: Send + Sized + 'static,
    {
        AsyncListenerTask::spawn(self)
    }
}

pub struct AsyncListenerTask {
    handle: JoinHandle<()>,
    token: CancellationToken,
}

impl AsyncListenerTask {
    /// Spawn listener through tokio::spawn with graceful cancellation ability.
    async fn spawn<T>(mut listener: T) -> AsyncListenerTask
    where
        T: AsyncListener + Send + 'static,
    {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let failure_msg = format!("[AsyncListenerTask] Failure occured while running {} listener", listener.name());

        let handle = tokio::spawn(async move {
            tokio::select! {
                res = listener.listen() => res.expect(&failure_msg),
                _ = token_clone.cancelled() => {
                    debug!(
                        "[AsyncListenerTask] {} listener has been cancelled. Shutting down the task ...",
                        listener.name()
                    );
                }
            }
        });

        yield_now().await;

        AsyncListenerTask { handle, token }
    }

    /// Cancel task and wait for it's termination.
    pub async fn cancel(self) -> Result<(), JoinError> {
        self.token.cancel();
        self.handle.await
    }
}

/*
 * Lurk HTTP endpoint listener
 */

pub struct LurkHttpEndpointListener {
    endpoint: LurkHttpEndpoint,
}

impl LurkHttpEndpointListener {
    pub fn new(addr: SocketAddr) -> LurkHttpEndpointListener {
        // Node is not running. Just instance is created.
        let node = LurkServer::new(SocketAddr::new(addr.ip(), 11222));
        // Create endpoint with lurk node passed.
        let endpoint = LurkHttpEndpoint::new(addr, Arc::new(node));

        LurkHttpEndpointListener { endpoint }
    }
}

impl AsyncListener for LurkHttpEndpointListener {
    fn listen(&mut self) -> impl Future<Output = Result<()>> + Send {
        self.endpoint.run()
    }

    fn name(&self) -> &'static str {
        "HTTP endpoint"
    }
}

/*
 * Lurk server listener
 */

pub struct LurkServerListener {
    server: LurkServer,
}

impl LurkServerListener {
    pub fn new(addr: SocketAddr) -> LurkServerListener {
        LurkServerListener {
            server: LurkServer::new(addr),
        }
    }
}

impl AsyncListener for LurkServerListener {
    fn listen(&mut self) -> impl Future<Output = Result<()>> + Send {
        self.server.run()
    }

    fn name(&self) -> &'static str {
        "Lurk server"
    }
}
