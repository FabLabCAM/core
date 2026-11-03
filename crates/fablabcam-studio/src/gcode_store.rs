//! Local profiles and immutable G-code revisions. No machine/network side effects.
mod archive;
mod files;
mod profiles;

pub use archive::{Archive, Revision};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Store { root: PathBuf }
impl Store {
    pub fn new(root: impl AsRef<Path>) -> Self { Self { root: root.as_ref().to_owned() } }
}
