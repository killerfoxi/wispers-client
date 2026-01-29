use super::handles::{
    WispersPendingNodeStateHandle, WispersRegisteredNodeStateHandle, complete_registration_internal,
};
use super::helpers::{c_str_to_string, reset_out_ptr};
use crate::errors::WispersStatus;
use crate::types::{AuthToken, ConnectivityGroupId, NodeRegistration};
use std::os::raw::{c_char, c_int};

#[unsafe(no_mangle)]
pub extern "C" fn wispers_pending_state_free(handle: *mut WispersPendingNodeStateHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wispers_registered_state_free(handle: *mut WispersRegisteredNodeStateHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wispers_pending_state_complete_registration(
    handle: *mut WispersPendingNodeStateHandle,
    connectivity_group_id: *const c_char,
    node_number: c_int,
    auth_token: *const c_char,
    out_registered: *mut *mut WispersRegisteredNodeStateHandle,
) -> WispersStatus {
    if handle.is_null() || out_registered.is_null() {
        return WispersStatus::NullPointer;
    }

    unsafe {
        reset_out_ptr(out_registered);
    }

    let connectivity = match c_str_to_string(connectivity_group_id) {
        Ok(value) => value,
        Err(err) => return err,
    };
    let token = match c_str_to_string(auth_token) {
        Ok(value) => value,
        Err(err) => return err,
    };

    let wrapper = unsafe { Box::from_raw(handle) };
    let registration = NodeRegistration::new(
        ConnectivityGroupId::from(connectivity),
        node_number,
        AuthToken::new(token),
    );

    match complete_registration_internal(wrapper.0, registration) {
        Ok(registered) => {
            let boxed = Box::new(WispersRegisteredNodeStateHandle(registered));
            unsafe {
                *out_registered = Box::into_raw(boxed);
            }
            WispersStatus::Success
        }
        Err(status) => status,
    }
}

// TODO: wispers_registered_state_logout_async - requires async FFI with callbacks
