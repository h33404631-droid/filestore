// Copyright 2025 Crrow
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{path::PathBuf, pin::Pin, sync::Arc, vec};

use async_trait::async_trait;
use futures::Stream;
use snafu::Snafu;
use value_log::Slice;

/// Errors that can occur during store operations.
#[derive(Debug, Snafu)]
pub enum Error {
    /// Key not found in the store.
    #[snafu(display("Key not found"))]
    KeyNotFound,

    /// I/O error occurred during store operation.
    #[snafu(display("I/O error: {source}"))]
    Io { source: std::io::Error },

    /// Store is in an invalid state or corrupted.
    #[snafu(display("Store corrupted: {message}"))]
    Corrupted { message: String },

    /// Operation not supported by this store implementation.
    #[snafu(display("Operation not supported: {operation}"))]
    Unsupported { operation: String },
}

pub type Result<T> = std::result::Result<T, Error>;

/// A reference to a store implementation.
pub type StoreRef = Arc<dyn Store>;

/// A type alias for async streams of key-value pairs.
pub type KeyStream = Pin<Box<dyn Stream<Item = Result<Slice>> + Send>>;

/// A storage abstraction that provides async key-value operations.
///
/// This trait defines the core interface for a key-value store with support for
/// basic CRUD operations and prefix scanning. All operations are async and
/// return `Result<T>` for proper error handling.
#[async_trait]
pub trait Store {
    /// Retrieves the value associated with the given key.
    ///
    /// # Arguments
    /// * `key` - The key to look up
    ///
    /// # Returns
    /// * `Ok(value)` - The value associated with the key
    /// * `Err(Error::KeyNotFound)` - If the key doesn't exist
    /// * `Err(_)` - Other storage errors
    async fn get(&self, key: Location) -> Result<Slice>;

    /// Stores a value in the store.
    ///
    /// # Arguments
    /// * `key` - The key to store
    /// * `value` - The value to associate with the key
    ///
    /// # Returns
    /// * `Ok(())` - If the operation succeeded
    /// * `Err(_)` - If the operation failed
    async fn put(&self, value: Slice) -> Result<Location>;

    /// Deletes multiple keys from the store.
    ///
    /// # Arguments
    /// * `keys` - A slice of keys to delete
    ///
    /// # Returns
    /// * `Ok(())` - If all deletions succeeded (missing keys are ignored)
    /// * `Err(_)` - If the operation failed
    async fn delete(&self, keys: Vec<Location>) -> Result<()>;

    /// Scans the store for all keys with the given prefix.
    ///
    /// Returns an async stream of keys that match the prefix. The stream
    /// yields `Result<Slice>` items, allowing for proper error handling
    /// during iteration.
    ///
    /// # Arguments
    /// * `prefix` - The prefix to search for
    ///
    /// # Returns
    /// * `Ok(stream)` - A stream of matching keys
    /// * `Err(_)` - If the scan operation failed to start
    async fn scan(&self, prefix: Location) -> Result<KeyStream>;
}

/// The location tells how to find the data in the store.
#[derive(Debug, derive_more::Deref, derive_more::AsRef, Clone)]
pub struct Location(u64);

#[derive(Debug)]
pub struct Options {
    pub path: PathBuf,
}

impl Options {
    pub fn open(self) -> Result<StoreRef> { todo!() }
}
