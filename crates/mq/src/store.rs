use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use memmap2::{Mmap, MmapMut, MmapOptions};
use rsketch_common::readable_size::ReadableSize;
use smart_default::SmartDefault;
use snafu::{ResultExt, Snafu};
use tokio::{fs::File, sync::Notify, time::timeout};

type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Snafu)]
pub(crate) enum StoreError {
    IO {
        source:   std::io::Error,
        #[snafu(implicit)]
        location: snafu::Location,
    },
    #[snafu(display("Close operation timed out after {timeout:?}"))]
    CloseTimeout { timeout: Duration },
}

#[derive(Debug, Clone, bon::Builder, SmartDefault)]
pub(crate) struct Options {
    pub(crate) file_path:     PathBuf,
    pub(crate) new:           bool,
    #[default(_code = "ReadableSize::gb(1)")]
    pub(crate) capacity:      ReadableSize,
    #[default(_code = "Duration::from_secs(5)")]
    pub(crate) close_timeout: Duration,
}

impl Options {
    pub(crate) async fn open(self) -> Result<QueueStore> {
        let mut file_options = tokio::fs::OpenOptions::new();
        file_options.read(true).write(true);
        if self.new {
            file_options.create(true).truncate(true);
        }
        let file = file_options.open(&self.file_path).await.context(IOSnafu)?;

        let store = if self.new {
            file.set_len(self.capacity.as_bytes() as u64)
                .await
                .context(IOSnafu)?;
            let mmap = unsafe {
                MmapOptions::new()
                    .offset(0)
                    .len(self.capacity.as_bytes() as usize)
                    .map_mut(&file)
                    .context(IOSnafu)?
            };
            QueueStore::Writable(QueueStoreInner::new(file, self, mmap))
        } else {
            let mmap = unsafe { Mmap::map(&file).context(IOSnafu)? };
            QueueStore::Readonly(QueueStoreInner::new(file, self, mmap))
        };
        Ok(store)
    }
}

pub(crate) enum QueueStore {
    Writable(QueueStoreInner<MmapMut>),
    Readonly(QueueStoreInner<Mmap>),
}

impl QueueStore {
    pub(crate) fn is_writable(&self) -> bool { matches!(self, Self::Writable(_)) }

    /// Apply a function to the inner store regardless of variant
    fn with_inner<R>(&self, f: impl Fn(&dyn QueueStoreOps) -> R) -> R {
        match self {
            Self::Writable(inner) => f(inner),
            Self::Readonly(inner) => f(inner),
        }
    }

    /// Apply an async function to the inner store regardless of variant  
    async fn with_inner_async<R, F, Fut>(&self, f: F) -> R
    where
        F: Fn(&dyn QueueStoreOps) -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        match self {
            Self::Writable(inner) => f(inner).await,
            Self::Readonly(inner) => f(inner).await,
        }
    }

    /// Apply an async function to the owned inner store
    async fn into_inner_async<R, F, Fut>(self, f: F) -> R
    where
        F: Fn(Box<dyn QueueStoreOpsOwned>) -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        match self {
            Self::Writable(inner) => f(Box::new(inner)).await,
            Self::Readonly(inner) => f(Box::new(inner)).await,
        }
    }

    /// Get the configured close timeout
    pub(crate) fn close_timeout(&self) -> Duration {
        self.with_inner(|inner| inner.close_timeout())
    }

    /// Close the store with the configured timeout
    pub(crate) async fn close(self) -> Result<()> {
        let timeout = self.close_timeout();
        self.close_with_timeout(timeout).await
    }

    /// Close the store with a custom timeout
    pub(crate) async fn close_with_timeout(self, timeout_duration: Duration) -> Result<()> {
        self.into_inner_async(|inner| inner.close_with_timeout(timeout_duration))
            .await
    }

    /// Create a reference handle for safe concurrent access
    pub(crate) fn create_handle(&self) -> QueueStoreHandle {
        match self {
            Self::Writable(inner) => QueueStoreHandle::Writable(inner.create_handle()),
            Self::Readonly(inner) => QueueStoreHandle::Readonly(inner.create_handle()),
        }
    }

    /// Check if the store is being closed
    pub(crate) fn is_closing(&self) -> bool { self.with_inner(|inner| inner.is_closing()) }
}

/// Common operations available on all queue stores
trait QueueStoreOps {
    fn close_timeout(&self) -> Duration;
    fn is_closing(&self) -> bool;
}

/// Operations that consume the queue store (like close)
trait QueueStoreOpsOwned {
    fn close_with_timeout(
        self: Box<Self>,
        timeout_duration: Duration,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

/// A generic queue store that can work with different mmap types.
pub(crate) struct QueueStoreInner<M> {
    // keep the file open to avoid the file being closed.
    _file:        File,
    mmap:         M,
    options:      Options,
    // Shared state for coordinating close operations
    shared_state: Arc<SharedState>,
}

struct SharedState {
    // ref_count is used to track the number of active references to the queue store.
    ref_count:    AtomicUsize,
    // When closing the file, we need to signal waiting operations.
    is_closing:   AtomicBool,
    // Notifier for async coordination during close operations
    close_notify: Notify,
}

impl<M> QueueStoreInner<M> {
    fn new(file: File, options: Options, mmap: M) -> Self {
        Self {
            _file: file,
            options,
            mmap,
            shared_state: Arc::new(SharedState {
                ref_count:    AtomicUsize::new(0),
                is_closing:   AtomicBool::new(false),
                close_notify: Notify::new(),
            }),
        }
    }

    /// Create a reference handle that automatically manages reference counting
    pub(crate) fn create_handle(&self) -> QueueStoreHandleInner<M> {
        // Increment reference count
        self.shared_state.ref_count.fetch_add(1, Ordering::Acquire);
        QueueStoreHandleInner {
            shared_state: Arc::clone(&self.shared_state),
            _phantom:     std::marker::PhantomData,
        }
    }

    /// Mark the store as closing and notify waiters
    fn mark_closing(&self) {
        self.shared_state.is_closing.store(true, Ordering::Release);
        self.shared_state.close_notify.notify_waiters();
    }

    /// Wait for all references to be dropped with a timeout
    async fn wait_for_references_with_timeout(&self, timeout_duration: Duration) -> Result<()> {
        let wait_future = async {
            while self.shared_state.ref_count.load(Ordering::Acquire) > 0 {
                self.shared_state.close_notify.notified().await;
            }
        };

        timeout(timeout_duration, wait_future)
            .await
            .map_err(|_| StoreError::CloseTimeout {
                timeout: timeout_duration,
            })?;

        Ok(())
    }
}

// Implement common operations trait for all QueueStoreInner types
impl<M> QueueStoreOps for QueueStoreInner<M> {
    fn close_timeout(&self) -> Duration { self.options.close_timeout }

    fn is_closing(&self) -> bool { self.shared_state.is_closing.load(Ordering::Acquire) }
}

// Implement owned operations for writable stores
impl QueueStoreOpsOwned for QueueStoreInner<MmapMut> {
    async fn close_with_timeout(self: Box<Self>, timeout_duration: Duration) -> Result<()> {
        self.mark_closing();
        self.wait_for_references_with_timeout(timeout_duration)
            .await?;

        // Flush the mmap before closing
        self.mmap.flush().context(IOSnafu)?;
        Ok(())
    }
}

// Implement owned operations for readonly stores
impl QueueStoreOpsOwned for QueueStoreInner<Mmap> {
    async fn close_with_timeout(self: Box<Self>, timeout_duration: Duration) -> Result<()> {
        self.mark_closing();
        self.wait_for_references_with_timeout(timeout_duration)
            .await?;
        Ok(())
    }
}

/// Enum wrapper for queue store handles
pub(crate) enum QueueStoreHandle {
    Writable(QueueStoreHandleInner<MmapMut>),
    Readonly(QueueStoreHandleInner<Mmap>),
}

impl QueueStoreHandle {
    /// Check if the store is being closed
    pub(crate) fn is_closing(&self) -> bool {
        match self {
            Self::Writable(handle) => handle.is_closing(),
            Self::Readonly(handle) => handle.is_closing(),
        }
    }
}

/// A handle that automatically manages reference counting for safe concurrent
/// access
pub(crate) struct QueueStoreHandleInner<M> {
    shared_state: Arc<SharedState>,
    _phantom:     std::marker::PhantomData<M>,
}

impl<M> Drop for QueueStoreHandleInner<M> {
    fn drop(&mut self) {
        // Decrement reference count and notify any waiting close operations
        if self.shared_state.ref_count.fetch_sub(1, Ordering::Release) == 1 {
            // This was the last reference, notify close operations
            self.shared_state.close_notify.notify_waiters();
        }
    }
}

impl<M> QueueStoreHandleInner<M> {
    /// Check if the store is being closed
    pub(crate) fn is_closing(&self) -> bool { self.shared_state.is_closing.load(Ordering::Acquire) }
}

// Type aliases for convenience
pub(crate) type WritableQueueStore = QueueStoreInner<MmapMut>;
pub(crate) type ReadonlyQueueStore = QueueStoreInner<Mmap>;

// Specialized implementations for specific mmap types
impl WritableQueueStore {
    /// Get mutable access to the memory map
    pub(crate) fn mmap_mut(&mut self) -> &mut MmapMut { &mut self.mmap }
}

impl ReadonlyQueueStore {
    /// Get read-only access to the memory map as a byte slice
    pub(crate) fn as_slice(&self) -> &[u8] { &self.mmap[..] }
}
