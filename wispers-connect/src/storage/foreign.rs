use crate::errors::WispersStatus;
use crate::storage::NodeStateStore;
use crate::types::{NodeRegistration, PersistedNodeState, RootKey};
use bincode;
use std::ffi::c_void;
use std::fmt;

const INITIAL_REGISTRATION_BUFFER: usize = 256;

/// Host-provided storage callbacks.
///
/// The `ctx` pointer carries all context the host needs, including any
/// namespace or isolation information. The library does not manage namespacing.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WispersNodeStorageCallbacks {
    pub ctx: *mut c_void,
    pub load_root_key:
        Option<unsafe extern "C" fn(ctx: *mut c_void, out: *mut u8, len: usize) -> WispersStatus>,
    pub save_root_key: Option<
        unsafe extern "C" fn(ctx: *mut c_void, key: *const u8, len: usize) -> WispersStatus,
    >,
    pub delete_root_key: Option<unsafe extern "C" fn(ctx: *mut c_void) -> WispersStatus>,
    pub load_registration: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            buf: *mut u8,
            len: usize,
            out_len: *mut usize,
        ) -> WispersStatus,
    >,
    pub save_registration: Option<
        unsafe extern "C" fn(ctx: *mut c_void, buf: *const u8, len: usize) -> WispersStatus,
    >,
    pub delete_registration: Option<unsafe extern "C" fn(ctx: *mut c_void) -> WispersStatus>,
}

unsafe impl Send for WispersNodeStorageCallbacks {}
unsafe impl Sync for WispersNodeStorageCallbacks {}

pub struct ForeignNodeStateStore {
    callbacks: WispersNodeStorageCallbacks,
}

#[derive(Debug)]
pub enum ForeignStoreError {
    MissingCallback(&'static str),
    RegistrationEncode,
    RegistrationDecode,
    Status(WispersStatus),
}

impl fmt::Display for ForeignStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForeignStoreError::MissingCallback(name) => write!(f, "missing callback: {name}"),
            ForeignStoreError::RegistrationEncode => write!(f, "failed to encode registration"),
            ForeignStoreError::RegistrationDecode => write!(f, "failed to decode registration"),
            ForeignStoreError::Status(status) => write!(f, "store callback returned {status:?}"),
        }
    }
}

impl std::error::Error for ForeignStoreError {}

impl ForeignNodeStateStore {
    pub fn new(callbacks: WispersNodeStorageCallbacks) -> Result<Self, ForeignStoreError> {
        if callbacks.load_root_key.is_none() {
            return Err(ForeignStoreError::MissingCallback("load_root_key"));
        }
        if callbacks.save_root_key.is_none() {
            return Err(ForeignStoreError::MissingCallback("save_root_key"));
        }
        if callbacks.delete_root_key.is_none() {
            return Err(ForeignStoreError::MissingCallback("delete_root_key"));
        }
        if callbacks.load_registration.is_none() {
            return Err(ForeignStoreError::MissingCallback("load_registration"));
        }
        if callbacks.save_registration.is_none() {
            return Err(ForeignStoreError::MissingCallback("save_registration"));
        }
        if callbacks.delete_registration.is_none() {
            return Err(ForeignStoreError::MissingCallback("delete_registration"));
        }

        Ok(Self { callbacks })
    }

    fn call_load_root_key(
        &self,
    ) -> Result<Option<[u8; crate::types::ROOT_KEY_LEN]>, ForeignStoreError> {
        let mut buffer = [0u8; crate::types::ROOT_KEY_LEN];
        let callback = self.callbacks.load_root_key.unwrap();
        let status =
            unsafe { callback(self.callbacks.ctx, buffer.as_mut_ptr(), buffer.len()) };
        match status {
            WispersStatus::Success => Ok(Some(buffer)),
            WispersStatus::NotFound => Ok(None),
            other => Err(ForeignStoreError::Status(other)),
        }
    }

    fn call_save_root_key(
        &self,
        root_key: &[u8; crate::types::ROOT_KEY_LEN],
    ) -> Result<(), ForeignStoreError> {
        let callback = self.callbacks.save_root_key.unwrap();
        let status = unsafe { callback(self.callbacks.ctx, root_key.as_ptr(), root_key.len()) };
        match status {
            WispersStatus::Success => Ok(()),
            other => Err(ForeignStoreError::Status(other)),
        }
    }

    fn call_delete_root_key(&self) -> Result<(), ForeignStoreError> {
        let callback = self.callbacks.delete_root_key.unwrap();
        let status = unsafe { callback(self.callbacks.ctx) };
        match status {
            WispersStatus::Success | WispersStatus::NotFound => Ok(()),
            other => Err(ForeignStoreError::Status(other)),
        }
    }

    fn call_load_registration(&self) -> Result<Option<NodeRegistration>, ForeignStoreError> {
        let callback = self.callbacks.load_registration.unwrap();
        let mut buffer = vec![0u8; INITIAL_REGISTRATION_BUFFER];
        let mut required = 0usize;

        loop {
            let status = unsafe {
                callback(
                    self.callbacks.ctx,
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    &mut required,
                )
            };

            match status {
                WispersStatus::Success => {
                    buffer.truncate(required);
                    return deserialize_registration(&buffer)
                        .map_err(|_| ForeignStoreError::RegistrationDecode);
                }
                WispersStatus::NotFound => return Ok(None),
                WispersStatus::BufferTooSmall => {
                    if required == 0 {
                        return Err(ForeignStoreError::Status(WispersStatus::BufferTooSmall));
                    }
                    buffer.resize(required, 0);
                }
                other => return Err(ForeignStoreError::Status(other)),
            }
        }
    }

    fn call_save_registration(
        &self,
        registration: Option<&NodeRegistration>,
    ) -> Result<(), ForeignStoreError> {
        let callback = self.callbacks.save_registration.unwrap();
        let bytes =
            serialize_registration(registration).map_err(|_| ForeignStoreError::RegistrationEncode)?;
        let status = unsafe { callback(self.callbacks.ctx, bytes.as_ptr(), bytes.len()) };
        match status {
            WispersStatus::Success => Ok(()),
            other => Err(ForeignStoreError::Status(other)),
        }
    }

    fn call_delete_registration(&self) -> Result<(), ForeignStoreError> {
        let callback = self.callbacks.delete_registration.unwrap();
        let status = unsafe { callback(self.callbacks.ctx) };
        match status {
            WispersStatus::Success | WispersStatus::NotFound => Ok(()),
            other => Err(ForeignStoreError::Status(other)),
        }
    }
}

unsafe impl Send for ForeignNodeStateStore {}
unsafe impl Sync for ForeignNodeStateStore {}

impl NodeStateStore for ForeignNodeStateStore {
    type Error = ForeignStoreError;

    fn load(&self) -> Result<Option<PersistedNodeState>, Self::Error> {
        let root_key = match self.call_load_root_key()? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        let registration = self.call_load_registration()?;
        Ok(Some(PersistedNodeState {
            root_key: RootKey::from_bytes(root_key),
            registration,
        }))
    }

    fn save(&self, state: &PersistedNodeState) -> Result<(), Self::Error> {
        self.call_save_root_key(state.root_key.as_bytes())?;
        self.call_save_registration(state.registration.as_ref())?;
        Ok(())
    }

    fn delete(&self) -> Result<(), Self::Error> {
        self.call_delete_root_key()?;
        self.call_delete_registration()?;
        Ok(())
    }
}

fn serialize_registration(registration: Option<&NodeRegistration>) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(&registration)
}

fn deserialize_registration(bytes: &[u8]) -> Result<Option<NodeRegistration>, bincode::Error> {
    bincode::deserialize(bytes)
}
