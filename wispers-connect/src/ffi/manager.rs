use super::handles::{ManagerImpl, WispersNodeStorageHandle};
use crate::state::NodeStorage;
use crate::storage::foreign::WispersNodeStateStoreCallbacks;
use crate::storage::{ForeignNodeStateStore, InMemoryNodeStateStore};

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

// TODO: wispers_storage_restore_or_init_async with callback-based API
// See discussion: FFI will use callbacks, Swift/Kotlin wrappers convert to native async
