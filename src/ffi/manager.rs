use super::handles::{
    ManagerImpl, WispersNodeStorageHandle, WispersPendingNodeStateHandle,
    WispersRegisteredNodeStateHandle, restore_or_init_internal,
};
use super::helpers::{c_str_to_string, optional_c_str, reset_out_ptr};
use crate::errors::WispersStatus;
use crate::state::NodeStorage;
use crate::storage::foreign::WispersNodeStateStoreCallbacks;
use crate::storage::{ForeignNodeStateStore, InMemoryNodeStateStore};
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
    callbacks: *const WispersNodeStateStoreCallbacks,
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
pub extern "C" fn wispers_storage_restore_or_init(
    handle: *mut WispersNodeStorageHandle,
    app_namespace: *const c_char,
    profile_namespace: *const c_char,
    out_pending: *mut *mut WispersPendingNodeStateHandle,
    out_registered: *mut *mut WispersRegisteredNodeStateHandle,
) -> WispersStatus {
    use super::handles::NodeStateStageImpl::{Pending, Registered};

    if handle.is_null() || out_pending.is_null() || out_registered.is_null() {
        return WispersStatus::NullPointer;
    }

    unsafe {
        reset_out_ptr(out_pending);
        reset_out_ptr(out_registered);
    }

    let app = match c_str_to_string(app_namespace) {
        Ok(value) => value,
        Err(err) => return err,
    };
    let profile = match optional_c_str(profile_namespace) {
        Ok(value) => value,
        Err(err) => return err,
    };

    let storage = unsafe { &mut (*handle).0 };
    match restore_or_init_internal(storage, app, profile) {
        Ok(Pending(pending)) => {
            let boxed = Box::new(WispersPendingNodeStateHandle(pending));
            unsafe {
                *out_pending = Box::into_raw(boxed);
            }
            WispersStatus::Success
        }
        Ok(Registered(registered)) => {
            let boxed = Box::new(WispersRegisteredNodeStateHandle(registered));
            unsafe {
                *out_registered = Box::into_raw(boxed);
            }
            WispersStatus::Success
        }
        Err(status) => status,
    }
}
