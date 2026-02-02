//! FFI functions for node lifecycle operations.
//!
//! This module contains all `wispers_storage_*` and `wispers_node_*` functions
//! for managing node storage, initialization, registration, activation, and logout.

use super::runtime;
use super::types::{
    c_str_to_string, CallbackContext, WispersCallback, WispersInitCallback, WispersNodeHandle,
    WispersNodeList, WispersNodeListCallback, WispersNodeState, WispersNodeStorageHandle,
    WispersRegistrationInfo,
};
use crate::errors::WispersStatus;
use crate::node::{NodeState, NodeStorage};
use crate::storage::foreign::WispersNodeStorageCallbacks;
use crate::storage::{ForeignNodeStateStore, InMemoryNodeStateStore};
use std::ffi::c_void;
use std::os::raw::c_char;

// =============================================================================
// Storage functions
// =============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn wispers_storage_new_in_memory() -> *mut WispersNodeStorageHandle {
    let storage = NodeStorage::new(InMemoryNodeStateStore::new());
    Box::into_raw(Box::new(WispersNodeStorageHandle(storage)))
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
    Box::into_raw(Box::new(WispersNodeStorageHandle(storage)))
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
        Err(_) => WispersStatus::StoreError,
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
    let storage = wrapper.0.clone();
    let ctx = CallbackContext(ctx);

    runtime::spawn(async move {
        let result = storage.restore_or_init_node().await;
        match result {
            Ok(node) => {
                let state = node_state_to_ffi(node.state());
                let handle = Box::into_raw(Box::new(WispersNodeHandle(node)));
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

    WispersStatus::Success
}

// =============================================================================
// Node functions
// =============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn wispers_node_free(handle: *mut WispersNodeHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

/// Get the current state/stage of the node.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_node_state(handle: *mut WispersNodeHandle) -> WispersNodeState {
    if handle.is_null() {
        return WispersNodeState::Pending;
    }

    let wrapper = unsafe { &*handle };
    node_state_to_ffi(wrapper.0.state())
}

/// Register the node with the hub using a registration token.
///
/// Returns INVALID_STATE if the node is not in Pending state.
/// The node handle is NOT consumed - it transitions to Registered state on success.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_node_register_async(
    handle: *mut WispersNodeHandle,
    token: *const c_char,
    ctx: *mut c_void,
    callback: WispersCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let token_str = match c_str_to_string(token) {
        Ok(s) => s,
        Err(status) => return status,
    };

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let ctx = CallbackContext(ctx);
    let handle_ptr = SendableNodePtr(handle);

    runtime::spawn(async move {
        // Safety: caller must ensure handle is valid and not used concurrently
        let wrapper = unsafe { handle_ptr.get_mut() };
        let result = wrapper.0.register(&token_str).await;

        let status = match result {
            Ok(()) => WispersStatus::Success,
            Err(e) => e.into(),
        };
        unsafe {
            callback(ctx.ptr(), status);
        }
    });

    WispersStatus::Success
}

/// Activate the node by pairing with an endorser.
///
/// The pairing code format is "node_number-secret" (e.g., "1-abc123xyz0").
/// Returns INVALID_STATE if the node is not in Registered state.
/// The node handle is NOT consumed - it transitions to Activated state on success.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_node_activate_async(
    handle: *mut WispersNodeHandle,
    pairing_code: *const c_char,
    ctx: *mut c_void,
    callback: WispersCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let pairing_code_str = match c_str_to_string(pairing_code) {
        Ok(s) => s,
        Err(status) => return status,
    };

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let ctx = CallbackContext(ctx);
    let handle_ptr = SendableNodePtr(handle);

    runtime::spawn(async move {
        // Safety: caller must ensure handle is valid and not used concurrently
        let wrapper = unsafe { handle_ptr.get_mut() };
        let result = wrapper.0.activate(&pairing_code_str).await;

        let status = match result {
            Ok(()) => WispersStatus::Success,
            Err(e) => e.into(),
        };
        unsafe {
            callback(ctx.ptr(), status);
        }
    });

    WispersStatus::Success
}

/// Logout the node (delete local state, deregister from hub if registered, revoke from roster if activated).
///
/// The node handle is CONSUMED by this call and must not be used afterward.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_node_logout_async(
    handle: *mut WispersNodeHandle,
    ctx: *mut c_void,
    callback: WispersCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    // Consume the handle
    let wrapper = unsafe { Box::from_raw(handle) };
    let ctx = CallbackContext(ctx);

    runtime::spawn(async move {
        let result = wrapper.0.logout().await;

        let status = match result {
            Ok(()) => WispersStatus::Success,
            Err(e) => e.into(),
        };
        unsafe {
            callback(ctx.ptr(), status);
        }
    });

    WispersStatus::Success
}

/// List all nodes in the connectivity group.
///
/// Returns INVALID_STATE if the node is in Pending state.
/// The node handle is NOT consumed.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_node_list_nodes_async(
    handle: *mut WispersNodeHandle,
    ctx: *mut c_void,
    callback: WispersNodeListCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let ctx = CallbackContext(ctx);
    let handle_ptr = SendableNodePtr(handle);

    runtime::spawn(async move {
        // Safety: caller must ensure handle is valid and not used concurrently
        let wrapper = unsafe { handle_ptr.get() };
        let result = wrapper.0.list_nodes().await.map_err(|e| e.into());
        handle_list_nodes_result(result, ctx, callback);
    });

    WispersStatus::Success
}

// =============================================================================
// Internal helpers
// =============================================================================

fn node_state_to_ffi(state: NodeState) -> WispersNodeState {
    match state {
        NodeState::Pending => WispersNodeState::Pending,
        NodeState::Registered => WispersNodeState::Registered,
        NodeState::Activated => WispersNodeState::Activated,
    }
}

/// Helper to send node pointer across threads.
///
/// Safety: The caller must ensure the handle remains valid and
/// is not accessed concurrently from other threads.
struct SendableNodePtr(*mut WispersNodeHandle);
unsafe impl Send for SendableNodePtr {}
unsafe impl Sync for SendableNodePtr {}

impl SendableNodePtr {
    /// Get an immutable reference to the inner handle.
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid.
    unsafe fn get(&self) -> &WispersNodeHandle {
        unsafe { &*self.0 }
    }

    /// Get a mutable reference to the inner handle.
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid.
    unsafe fn get_mut(&self) -> &mut WispersNodeHandle {
        unsafe { &mut *self.0 }
    }
}

fn handle_list_nodes_result(
    result: Result<Vec<crate::types::NodeInfo>, WispersStatus>,
    ctx: CallbackContext,
    callback: unsafe extern "C" fn(*mut c_void, WispersStatus, *mut WispersNodeList),
) {
    match result {
        Ok(nodes) => match WispersNodeList::from_node_infos(nodes) {
            Ok(list) => {
                let list_ptr = Box::into_raw(Box::new(list));
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success, list_ptr);
                }
            }
            Err(status) => {
                unsafe {
                    callback(ctx.ptr(), status, std::ptr::null_mut());
                }
            }
        },
        Err(status) => {
            unsafe {
                callback(ctx.ptr(), status, std::ptr::null_mut());
            }
        }
    }
}
