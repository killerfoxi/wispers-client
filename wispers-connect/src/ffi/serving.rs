//! FFI bindings for serving sessions.

use super::callbacks::CallbackContext;
use super::runtime;
use crate::errors::WispersStatus;
use crate::serving::{IncomingConnections, ServingHandle, ServingSession};
use std::ffi::{c_void, CString};
use std::os::raw::c_char;

/// Opaque handle to a serving command interface.
///
/// Use this to generate pairing codes and control the session.
/// This handle can be cloned internally and remains valid until freed.
pub struct WispersServingHandle(pub(crate) ServingHandle);

/// Opaque handle to a serving session runner.
///
/// Pass this to `wispers_serving_session_run_async` to start the event loop.
/// The session is consumed when run starts.
pub struct WispersServingSession(pub(crate) Option<ServingSession>);

/// Opaque handle to incoming P2P connection receivers.
///
/// Only present for activated nodes (not registered nodes).
pub struct WispersIncomingConnections(pub(crate) IncomingConnections);

// Callback types for serving operations

/// Callback for start_serving that receives the session components.
pub type WispersStartServingCallback = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        status: WispersStatus,
        serving_handle: *mut WispersServingHandle,
        session: *mut WispersServingSession,
        incoming: *mut WispersIncomingConnections,
    ),
>;

/// Callback that receives a pairing code string.
pub type WispersPairingCodeCallback = Option<
    unsafe extern "C" fn(ctx: *mut c_void, status: WispersStatus, pairing_code: *mut c_char),
>;

// Free functions

#[unsafe(no_mangle)]
pub extern "C" fn wispers_serving_handle_free(handle: *mut WispersServingHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wispers_serving_session_free(handle: *mut WispersServingSession) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wispers_incoming_connections_free(handle: *mut WispersIncomingConnections) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

// Start serving functions

/// Start a serving session for a registered node.
///
/// Registered nodes can serve for bootstrapping but cannot accept P2P connections.
/// The callback receives the serving handle and session (incoming will be NULL).
/// The registered handle is NOT consumed.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_registered_node_start_serving_async(
    handle: *mut super::handles::WispersRegisteredNodeHandle,
    ctx: *mut c_void,
    callback: WispersStartServingCallback,
) -> WispersStatus {
    use super::handles::RegisteredImpl;

    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let wrapper = unsafe { &*handle };
    let ctx = CallbackContext(ctx);

    // Extract what we need before spawning
    let (hub_addr, registration, root_key) = match &wrapper.0 {
        RegisteredImpl::InMemory(registered) => (
            registered.hub_addr(),
            registered.registration().clone(),
            get_root_key_registered(registered),
        ),
        RegisteredImpl::Foreign(registered) => (
            registered.hub_addr(),
            registered.registration().clone(),
            get_root_key_registered(registered),
        ),
    };

    runtime::spawn(async move {
        let result = start_serving_registered_impl(&hub_addr, &registration, &root_key).await;
        match result {
            Ok((serving_handle, session)) => {
                let h = Box::into_raw(Box::new(WispersServingHandle(serving_handle)));
                let s = Box::into_raw(Box::new(WispersServingSession(Some(session))));
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success, h, s, std::ptr::null_mut());
                }
            }
            Err(_) => {
                unsafe {
                    callback(
                        ctx.ptr(),
                        WispersStatus::HubError,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    );
                }
            }
        }
    });

    WispersStatus::Success
}

/// Start a serving session for an activated node.
///
/// Activated nodes can accept P2P connections. The callback receives the serving handle,
/// session, and incoming connections handle.
/// The activated handle is NOT consumed.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_activated_node_start_serving_async(
    handle: *mut super::handles::WispersActivatedNodeHandle,
    ctx: *mut c_void,
    callback: WispersStartServingCallback,
) -> WispersStatus {
    use super::handles::ActivatedImpl;

    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let wrapper = unsafe { &*handle };
    let ctx = CallbackContext(ctx);

    // Extract what we need before spawning
    let (hub_addr, registration, signing_key, x25519_key) = match &wrapper.0 {
        ActivatedImpl::InMemory(activated) => (
            activated.hub_addr(),
            activated.registration().clone(),
            activated.signing_key().clone(),
            get_x25519_key_activated(activated),
        ),
        ActivatedImpl::Foreign(activated) => (
            activated.hub_addr(),
            activated.registration().clone(),
            activated.signing_key().clone(),
            get_x25519_key_activated(activated),
        ),
    };

    runtime::spawn(async move {
        let result =
            start_serving_activated_impl(&hub_addr, &registration, signing_key, x25519_key).await;
        match result {
            Ok((serving_handle, session, incoming)) => {
                let h = Box::into_raw(Box::new(WispersServingHandle(serving_handle)));
                let s = Box::into_raw(Box::new(WispersServingSession(Some(session))));
                let i = incoming
                    .map(|inc| Box::into_raw(Box::new(WispersIncomingConnections(inc))))
                    .unwrap_or(std::ptr::null_mut());
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success, h, s, i);
                }
            }
            Err(_) => {
                unsafe {
                    callback(
                        ctx.ptr(),
                        WispersStatus::HubError,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    );
                }
            }
        }
    });

    WispersStatus::Success
}

/// Generate a pairing code for endorsing a new node.
///
/// The serving handle is NOT consumed.
/// On success, the callback receives the pairing code string (caller must free with wispers_string_free).
#[unsafe(no_mangle)]
pub extern "C" fn wispers_serving_handle_generate_pairing_code_async(
    handle: *mut WispersServingHandle,
    ctx: *mut c_void,
    callback: WispersPairingCodeCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let wrapper = unsafe { &*handle };
    let serving_handle = wrapper.0.clone();
    let ctx = CallbackContext(ctx);

    runtime::spawn(async move {
        let result = serving_handle.generate_pairing_secret().await;
        match result {
            Ok(pairing_code) => {
                let code_str = pairing_code.format();
                match CString::new(code_str) {
                    Ok(cstr) => {
                        unsafe {
                            callback(ctx.ptr(), WispersStatus::Success, cstr.into_raw());
                        }
                    }
                    Err(_) => {
                        unsafe {
                            callback(ctx.ptr(), WispersStatus::InvalidUtf8, std::ptr::null_mut());
                        }
                    }
                }
            }
            Err(_) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::HubError, std::ptr::null_mut());
                }
            }
        }
    });

    WispersStatus::Success
}

/// Run the serving session event loop.
///
/// The session handle is CONSUMED by this call.
/// The callback is invoked when the session ends (either by shutdown or error).
#[unsafe(no_mangle)]
pub extern "C" fn wispers_serving_session_run_async(
    handle: *mut WispersServingSession,
    ctx: *mut c_void,
    callback: super::callbacks::WispersCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    // Consume the session
    let mut wrapper = unsafe { Box::from_raw(handle) };
    let session = match wrapper.0.take() {
        Some(s) => s,
        None => {
            // Session was already consumed
            return WispersStatus::UnexpectedStage;
        }
    };
    let ctx = CallbackContext(ctx);

    runtime::spawn(async move {
        let result = session.run().await;
        let status = match result {
            Ok(()) => WispersStatus::Success,
            Err(_) => WispersStatus::HubError,
        };
        unsafe {
            callback(ctx.ptr(), status);
        }
    });

    WispersStatus::Success
}

/// Request the serving session to shut down.
///
/// The serving handle is NOT consumed.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_serving_handle_shutdown_async(
    handle: *mut WispersServingHandle,
    ctx: *mut c_void,
    callback: super::callbacks::WispersCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let wrapper = unsafe { &*handle };
    let serving_handle = wrapper.0.clone();
    let ctx = CallbackContext(ctx);

    runtime::spawn(async move {
        let result = serving_handle.shutdown().await;
        let status = match result {
            Ok(()) => WispersStatus::Success,
            Err(_) => WispersStatus::HubError,
        };
        unsafe {
            callback(ctx.ptr(), status);
        }
    });

    WispersStatus::Success
}

// Implementation helpers

async fn start_serving_registered_impl(
    hub_addr: &str,
    registration: &crate::types::NodeRegistration,
    root_key: &[u8; 32],
) -> Result<(ServingHandle, ServingSession), crate::hub::HubError> {
    use crate::crypto::SigningKeyPair;
    use crate::hub::HubClient;
    use crate::serving::ServingSession;

    let signing_key = SigningKeyPair::derive_from_root_key(root_key);
    let mut client = HubClient::connect(hub_addr).await?;
    let conn = client.start_serving(registration).await?;

    let (handle, session, _incoming) = ServingSession::new(
        conn,
        signing_key,
        registration.connectivity_group_id.clone(),
        registration.node_number,
        None, // No P2P for registered nodes
    );

    Ok((handle, session))
}

async fn start_serving_activated_impl(
    hub_addr: &str,
    registration: &crate::types::NodeRegistration,
    signing_key: crate::crypto::SigningKeyPair,
    x25519_key: crate::crypto::X25519KeyPair,
) -> Result<(ServingHandle, ServingSession, Option<IncomingConnections>), crate::hub::HubError> {
    use crate::hub::HubClient;
    use crate::serving::{P2pConfig, ServingSession};

    let mut client = HubClient::connect(hub_addr).await?;
    let conn = client.start_serving(registration).await?;

    let p2p_config = P2pConfig {
        x25519_key,
        hub_addr: hub_addr.to_string(),
        registration: registration.clone(),
    };

    let (handle, session, incoming) = ServingSession::new(
        conn,
        signing_key,
        registration.connectivity_group_id.clone(),
        registration.node_number,
        Some(p2p_config),
    );

    Ok((handle, session, incoming))
}

// Accessors using the public methods we added to state types

fn get_root_key_registered<S: crate::storage::NodeStateStore>(
    registered: &crate::state::RegisteredNodeState<S>,
) -> [u8; 32] {
    *registered.root_key_bytes()
}

fn get_x25519_key_activated<S: crate::storage::NodeStateStore>(
    activated: &crate::state::ActivatedNode<S>,
) -> crate::crypto::X25519KeyPair {
    activated.x25519_key().clone()
}
