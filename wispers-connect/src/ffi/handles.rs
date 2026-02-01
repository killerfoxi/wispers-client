use crate::errors::{NodeStateError, WispersStatus};
use crate::node::Node;
use crate::state::NodeStorage;
use crate::storage::InMemoryStoreError;
use crate::storage::{ForeignNodeStateStore, InMemoryNodeStateStore, foreign::ForeignStoreError};

pub enum ManagerImpl {
    InMemory(NodeStorage<InMemoryNodeStateStore>),
    Foreign(NodeStorage<ForeignNodeStateStore>),
}

pub enum NodeImpl {
    InMemory(Node<InMemoryNodeStateStore>),
    Foreign(Node<ForeignNodeStateStore>),
}

pub struct WispersNodeStorageHandle(pub ManagerImpl);
pub struct WispersNodeHandle(pub NodeImpl);

impl From<NodeStateError<InMemoryStoreError>> for WispersStatus {
    fn from(value: NodeStateError<InMemoryStoreError>) -> Self {
        match value {
            NodeStateError::Store(_) => WispersStatus::StoreError,
            NodeStateError::Hub(_) => WispersStatus::StoreError, // TODO: add proper status
            NodeStateError::AlreadyRegistered => WispersStatus::AlreadyRegistered,
            NodeStateError::NotRegistered => WispersStatus::NotRegistered,
            NodeStateError::InvalidPairingCode(_) => WispersStatus::InvalidPairingCode,
            NodeStateError::MacVerificationFailed => WispersStatus::ActivationFailed,
            NodeStateError::MissingEndorserResponse => WispersStatus::ActivationFailed,
            NodeStateError::RosterVerificationFailed(_) => WispersStatus::ActivationFailed,
            NodeStateError::InvalidState { .. } => WispersStatus::InvalidState,
        }
    }
}

impl From<NodeStateError<ForeignStoreError>> for WispersStatus {
    fn from(value: NodeStateError<ForeignStoreError>) -> Self {
        match value {
            NodeStateError::Store(ForeignStoreError::Status(status)) => status,
            NodeStateError::Store(ForeignStoreError::MissingCallback(_)) => {
                WispersStatus::MissingCallback
            }
            NodeStateError::Store(
                ForeignStoreError::RegistrationEncode | ForeignStoreError::RegistrationDecode,
            ) => WispersStatus::StoreError,
            NodeStateError::Hub(_) => WispersStatus::StoreError, // TODO: add proper status
            NodeStateError::AlreadyRegistered => WispersStatus::AlreadyRegistered,
            NodeStateError::NotRegistered => WispersStatus::NotRegistered,
            NodeStateError::InvalidPairingCode(_) => WispersStatus::InvalidPairingCode,
            NodeStateError::MacVerificationFailed => WispersStatus::ActivationFailed,
            NodeStateError::MissingEndorserResponse => WispersStatus::ActivationFailed,
            NodeStateError::RosterVerificationFailed(_) => WispersStatus::ActivationFailed,
            NodeStateError::InvalidState { .. } => WispersStatus::InvalidState,
        }
    }
}
