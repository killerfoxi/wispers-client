use crate::errors::{NodeStateError, WispersStatus};
use crate::state::{NodeStorage, PendingNodeState, RegisteredNodeState};
use crate::storage::InMemoryStoreError;
use crate::storage::{ForeignNodeStateStore, InMemoryNodeStateStore, foreign::ForeignStoreError};
use crate::types::NodeRegistration;

pub enum ManagerImpl {
    InMemory(NodeStorage<InMemoryNodeStateStore>),
    Foreign(NodeStorage<ForeignNodeStateStore>),
}

pub enum PendingImpl {
    InMemory(PendingNodeState<InMemoryNodeStateStore>),
    Foreign(PendingNodeState<ForeignNodeStateStore>),
}

pub enum RegisteredImpl {
    InMemory(RegisteredNodeState<InMemoryNodeStateStore>),
    Foreign(RegisteredNodeState<ForeignNodeStateStore>),
}

pub struct WispersNodeStorageHandle(pub ManagerImpl);
pub struct WispersPendingNodeStateHandle(pub PendingImpl);
pub struct WispersRegisteredNodeStateHandle(pub RegisteredImpl);

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
                ForeignStoreError::CStringConversion
                | ForeignStoreError::RegistrationEncode
                | ForeignStoreError::RegistrationDecode,
            ) => WispersStatus::StoreError,
            NodeStateError::Hub(_) => WispersStatus::StoreError, // TODO: add proper status
            NodeStateError::AlreadyRegistered => WispersStatus::AlreadyRegistered,
            NodeStateError::NotRegistered => WispersStatus::NotRegistered,
            NodeStateError::InvalidPairingCode(_) => WispersStatus::InvalidPairingCode,
            NodeStateError::MacVerificationFailed => WispersStatus::ActivationFailed,
            NodeStateError::MissingEndorserResponse => WispersStatus::ActivationFailed,
        }
    }
}

// TODO: restore_or_init requires async FFI with callbacks (deferred)
// TODO: logout requires async FFI with callbacks (deferred)

pub fn complete_registration_internal(
    pending: PendingImpl,
    registration: NodeRegistration,
) -> Result<RegisteredImpl, WispersStatus> {
    match pending {
        PendingImpl::InMemory(inner) => inner
            .complete_registration(registration)
            .map(RegisteredImpl::InMemory)
            .map_err(Into::into),
        PendingImpl::Foreign(inner) => inner
            .complete_registration(registration)
            .map(RegisteredImpl::Foreign)
            .map_err(Into::into),
    }
}

pub fn registration_url_internal(pending: &PendingImpl, base_url: &str) -> String {
    match pending {
        PendingImpl::InMemory(inner) => inner.registration_url(base_url),
        PendingImpl::Foreign(inner) => inner.registration_url(base_url),
    }
}
