use crate::storage::NodeStateStore;
use crate::types::PersistedNodeState;
use std::sync::RwLock;
use thiserror::Error;

/// Simple, non-persistent store useful for testing and sketches.
#[derive(Default)]
pub struct InMemoryNodeStateStore {
    state: RwLock<Option<PersistedNodeState>>,
}

impl InMemoryNodeStateStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl NodeStateStore for InMemoryNodeStateStore {
    type Error = InMemoryStoreError;

    fn load(&self) -> Result<Option<PersistedNodeState>, Self::Error> {
        let state = self.state.read().map_err(|_| InMemoryStoreError::Poisoned)?;
        Ok(state.clone())
    }

    fn save(&self, state: &PersistedNodeState) -> Result<(), Self::Error> {
        let mut stored = self.state.write().map_err(|_| InMemoryStoreError::Poisoned)?;
        *stored = Some(state.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), Self::Error> {
        let mut stored = self.state.write().map_err(|_| InMemoryStoreError::Poisoned)?;
        *stored = None;
        Ok(())
    }
}

/// Errors that can arise from the in-memory store (primarily poisoning).
#[derive(Debug, Error)]
pub enum InMemoryStoreError {
    #[error("in-memory state lock was poisoned")]
    Poisoned,
}
