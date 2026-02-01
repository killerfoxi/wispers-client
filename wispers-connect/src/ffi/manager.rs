use super::callbacks::{CallbackContext, WispersInitCallback, WispersNodeState};
use super::handles::{ManagerImpl, NodeImpl, WispersNodeHandle, WispersNodeStorageHandle};
use super::helpers::{c_str_to_string, WispersRegistrationInfo};
use super::runtime;
use crate::errors::WispersStatus;
use crate::node::NodeState;
use crate::state::NodeStorage;
use crate::storage::foreign::WispersNodeStorageCallbacks;
use crate::storage::{ForeignNodeStateStore, InMemoryNodeStateStore};
use std::ffi::c_void;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn wispers_storage_new_in_memory() -> *mut WispersNodeStorageHandle {
    let storage = NodeStorage::new(InMemoryNodeStateStore::new());
    Box::into_raw(Box::new(WispersNodeStorageHandle(ManagerImpl::InMemory(
        storage,
    ))))
}

#[unsafe(no_mangle)]
pub extern "C" fn wispers_storage_new_with_callbacks(
    callbacks: *const WispersNodeStorageCallbacks,
) -> *mut WispersNodeStorageHandle {
    if callbacks.is_null() {
        return std::ptr::null_mut();
    }

    let callbacks = unsafe { *callbacks };
    let store = match ForeignNodeStateStore::new(callbacks) {
        Ok(store) => store,
        Err(_) => return std::ptr::null_mut(),
    };
    let storage = NodeStorage::new(store);
    Box::into_raw(Box::new(WispersNodeStorageHandle(ManagerImpl::Foreign(
        storage,
    ))))
}

#[unsafe(no_mangle)]
pub extern "C" fn wispers_storage_free(handle: *mut WispersNodeStorageHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wispers_storage_read_registration(
    handle: *mut WispersNodeStorageHandle,
    out_info: *mut WispersRegistrationInfo,
) -> WispersStatus {
    if handle.is_null() || out_info.is_null() {
        return WispersStatus::NullPointer;
    }

    let wrapper = unsafe { &*handle };
    let maybe_reg = wrapper.0.read_registration();

    match maybe_reg {
        Ok(Some(reg)) => match WispersRegistrationInfo::from_registration(&reg) {
            Ok(info) => {
                unsafe { *out_info = info };
                WispersStatus::Success
            }
            Err(status) => status,
        },
        Ok(None) => {
            unsafe { *out_info = WispersRegistrationInfo::null() };
            WispersStatus::NotFound
        }
        Err(status) => status,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wispers_storage_override_hub_addr(
    handle: *mut WispersNodeStorageHandle,
    hub_addr: *const c_char,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let addr = match c_str_to_string(hub_addr) {
        Ok(s) => s,
        Err(status) => return status,
    };

    let wrapper = unsafe { &*handle };
    wrapper.0.override_hub_addr(addr);

    WispersStatus::Success
}

/// Restore or initialize node state asynchronously.
///
/// On success, the callback receives a single node handle and the current state.
/// The storage handle remains valid and is NOT consumed by this call.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_storage_restore_or_init_async(
    handle: *mut WispersNodeStorageHandle,
    ctx: *mut c_void,
    callback: WispersInitCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let wrapper = unsafe { &*handle };
    let ctx = CallbackContext(ctx);

    wrapper.0.restore_or_init_async(ctx, callback);

    WispersStatus::Success
}

// Helper methods on ManagerImpl to reduce duplication
impl ManagerImpl {
    fn read_registration(&self) -> Result<Option<crate::types::NodeRegistration>, WispersStatus> {
        match self {
            ManagerImpl::InMemory(storage) => storage
                .read_registration()
                .map_err(|_| WispersStatus::StoreError),
            ManagerImpl::Foreign(storage) => storage
                .read_registration()
                .map_err(|_| WispersStatus::StoreError),
        }
    }

    fn override_hub_addr(&self, addr: String) {
        match self {
            ManagerImpl::InMemory(storage) => storage.override_hub_addr(addr),
            ManagerImpl::Foreign(storage) => storage.override_hub_addr(addr),
        }
    }

    fn restore_or_init_async(
        &self,
        ctx: CallbackContext,
        callback: unsafe extern "C" fn(
            *mut c_void,
            WispersStatus,
            *mut WispersNodeHandle,
            WispersNodeState,
        ),
    ) {
        match self {
            ManagerImpl::InMemory(storage) => {
                let storage = storage.clone();
                runtime::spawn(async move {
                    let result = storage.restore_or_init_node().await;
                    match result {
                        Ok(node) => {
                            let state = node_state_to_ffi(node.state());
                            let handle = Box::into_raw(Box::new(WispersNodeHandle(
                                NodeImpl::InMemory(node),
                            )));
                            unsafe {
                                callback(ctx.ptr(), WispersStatus::Success, handle, state);
                            }
                        }
                        Err(e) => {
                            let status: WispersStatus = e.into();
                            unsafe {
                                callback(
                                    ctx.ptr(),
                                    status,
                                    std::ptr::null_mut(),
                                    WispersNodeState::Pending,
                                );
                            }
                        }
                    }
                });
            }
            ManagerImpl::Foreign(storage) => {
                let storage = storage.clone();
                runtime::spawn(async move {
                    let result = storage.restore_or_init_node().await;
                    match result {
                        Ok(node) => {
                            let state = node_state_to_ffi(node.state());
                            let handle = Box::into_raw(Box::new(WispersNodeHandle(
                                NodeImpl::Foreign(node),
                            )));
                            unsafe {
                                callback(ctx.ptr(), WispersStatus::Success, handle, state);
                            }
                        }
                        Err(e) => {
                            let status: WispersStatus = e.into();
                            unsafe {
                                callback(
                                    ctx.ptr(),
                                    status,
                                    std::ptr::null_mut(),
                                    WispersNodeState::Pending,
                                );
                            }
                        }
                    }
                });
            }
        }
    }
}

fn node_state_to_ffi(state: NodeState) -> WispersNodeState {
    match state {
        NodeState::Pending => WispersNodeState::Pending,
        NodeState::Registered => WispersNodeState::Registered,
        NodeState::Activated => WispersNodeState::Activated,
    }
}
