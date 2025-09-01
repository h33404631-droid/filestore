use tokio::io::AsyncWrite;

/// A writer for the data portion of the store.
pub(crate) struct DataWriter<W: AsyncWrite> {
    writer: W,
}

impl<W: AsyncWrite> DataWriter<W> {
    pub(crate) fn open(writer: W) -> Self { Self { writer } }
}
