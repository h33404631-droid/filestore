use std::sync::Arc;

use async_trait::async_trait;
use value_log::Slice;

use super::err::Result;

pub type Index = u64;
pub type Cycle = u64;

pub type AppenderRef = Arc<dyn Appender>;

#[async_trait]
pub trait Appender {
    // Append the data to the appender.
    async fn write(&self, data: Slice) -> Result<()>;
    // Returns the index last written.
    async fn last_index(&self) -> Result<Index>;
    // Returns the cycle of the appender.
    fn cycle(&self) -> u64;
}
