use std::path::PathBuf;

use super::Result;

pub(crate) struct Config {
    root: PathBuf,
}

impl Config {
    pub(crate) fn open(self) -> Result<MmapStore> { todo!() }
}

pub(crate) struct MmapStore {}

impl MmapStore {}
