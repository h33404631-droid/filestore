use crate::{appender::AppenderRef, err::Result};

pub trait QueueAPI {
    async fn acquire(&self) -> Result<AppenderRef>;
}

struct SinlgeQueue {}
