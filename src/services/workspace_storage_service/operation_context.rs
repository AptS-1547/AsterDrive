use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};

use crate::errors::Result;

pub(crate) trait StorageCancellationCheck: Send + Sync {
    fn checkpoint(&self) -> Result<()>;
}

impl<F> StorageCancellationCheck for F
where
    F: Fn() -> Result<()> + Send + Sync,
{
    fn checkpoint(&self) -> Result<()> {
        self()
    }
}

#[derive(Clone, Default)]
pub(crate) struct StorageOperationContext {
    cancellation: Option<Arc<dyn StorageCancellationCheck>>,
}

impl StorageOperationContext {
    pub(crate) fn new<C>(cancellation: C) -> Self
    where
        C: StorageCancellationCheck + 'static,
    {
        Self {
            cancellation: Some(Arc::new(cancellation)),
        }
    }

    pub(crate) fn checkpoint(&self) -> Result<()> {
        if let Some(cancellation) = &self.cancellation {
            cancellation.checkpoint()?;
        }
        Ok(())
    }

    pub(crate) fn is_cancellable(&self) -> bool {
        self.cancellation.is_some()
    }

    pub(crate) fn wrap_reader(
        &self,
        reader: Box<dyn AsyncRead + Unpin + Send>,
    ) -> Box<dyn AsyncRead + Unpin + Send + Sync> {
        let reader: Box<dyn AsyncRead + Unpin + Send + Sync> = Box::new(SyncRead::new(reader));
        match &self.cancellation {
            Some(cancellation) => Box::new(CancellationAwareReader {
                inner: reader,
                cancellation: cancellation.clone(),
            }),
            None => reader,
        }
    }
}

struct SyncRead {
    inner: Mutex<Box<dyn AsyncRead + Unpin + Send>>,
}

impl SyncRead {
    fn new(inner: Box<dyn AsyncRead + Unpin + Send>) -> Self {
        Self {
            inner: Mutex::new(inner),
        }
    }
}

impl AsyncRead for SyncRead {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.inner.lock() {
            Ok(mut inner) => Pin::new(&mut *inner).poll_read(cx, buf),
            Err(_) => Poll::Ready(Err(std::io::Error::other("sync reader mutex poisoned"))),
        }
    }
}

impl Unpin for SyncRead {}

struct CancellationAwareReader {
    inner: Box<dyn AsyncRead + Unpin + Send + Sync>,
    cancellation: Arc<dyn StorageCancellationCheck>,
}

impl AsyncRead for CancellationAwareReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if let Err(error) = self.cancellation.checkpoint() {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                error.to_string(),
            )));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl Unpin for CancellationAwareReader {}
