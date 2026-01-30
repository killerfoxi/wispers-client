use crate::types::NodeState;
use std::sync::Arc;

pub mod file;
pub mod foreign;
pub mod in_memory;

pub use file::{FileNodeStateStore, FileStoreError};
pub use foreign::{ForeignNodeStateStore, ForeignStoreError, WispersNodeStorageCallbacks};
pub use in_memory::{InMemoryNodeStateStore, InMemoryStoreError};

/// Storage backend for node state.
///
/// Implementations are responsible for their own namespacing/isolation.
/// The library treats each store instance as storing exactly one node's state.
pub trait NodeStateStore: Send + Sync + 'static {
    type Error;

    fn load(&self) -> Result<Option<NodeState>, Self::Error>;

    fn save(&self, state: &NodeState) -> Result<(), Self::Error>;

    fn delete(&self) -> Result<(), Self::Error>;
}

pub(crate) type SharedStore<S> = Arc<S>;
