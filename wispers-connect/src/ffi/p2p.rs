//! FFI bindings for P2P connections.

use super::callbacks::CallbackContext;
use super::handles::ActivatedImpl;
use super::runtime;
use crate::errors::WispersStatus;
use crate::p2p::{QuicConnection, QuicStream, UdpConnection};
use std::ffi::c_void;
use std::os::raw::c_int;

// Helpers to send raw pointers across threads

struct SendableUdpConnPtr(*mut WispersUdpConnectionHandle);
unsafe impl Send for SendableUdpConnPtr {}

impl SendableUdpConnPtr {
    unsafe fn get(&self) -> &UdpConnection {
        unsafe { &(*self.0).0 }
    }
}

struct SendableActivatedPtr(*mut super::handles::WispersActivatedNodeHandle);
unsafe impl Send for SendableActivatedPtr {}

impl SendableActivatedPtr {
    unsafe fn get(&self) -> &super::handles::WispersActivatedNodeHandle {
        unsafe { &*self.0 }
    }
}

/// Opaque handle to a UDP P2P connection.
pub struct WispersUdpConnectionHandle(pub(crate) UdpConnection);

/// Callback that receives a UDP connection handle.
pub type WispersUdpConnectionCallback = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        status: WispersStatus,
        connection: *mut WispersUdpConnectionHandle,
    ),
>;

/// Callback that receives received data.
pub type WispersDataCallback = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        status: WispersStatus,
        data: *const u8,
        len: usize,
    ),
>;

// Free function

#[unsafe(no_mangle)]
pub extern "C" fn wispers_udp_connection_free(handle: *mut WispersUdpConnectionHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

/// Connect to a peer node using UDP transport.
///
/// The activated handle is NOT consumed.
/// On success, callback receives the UDP connection handle.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_activated_node_connect_udp_async(
    handle: *mut super::handles::WispersActivatedNodeHandle,
    peer_node_number: c_int,
    ctx: *mut c_void,
    callback: WispersUdpConnectionCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let ctx = CallbackContext(ctx);
    let activated_ptr = SendableActivatedPtr(handle);

    runtime::spawn(async move {
        let wrapper = unsafe { activated_ptr.get() };
        let result = match &wrapper.0 {
            ActivatedImpl::InMemory(activated) => activated.connect_udp(peer_node_number).await,
            ActivatedImpl::Foreign(activated) => activated.connect_udp(peer_node_number).await,
        };

        match result {
            Ok(conn) => {
                let h = Box::into_raw(Box::new(WispersUdpConnectionHandle(conn)));
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success, h);
                }
            }
            Err(_) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::ConnectionFailed, std::ptr::null_mut());
                }
            }
        }
    });

    WispersStatus::Success
}

/// Send data over a UDP connection.
///
/// This is a synchronous, non-blocking operation.
/// The connection handle is NOT consumed.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_udp_connection_send(
    handle: *mut WispersUdpConnectionHandle,
    data: *const u8,
    len: usize,
) -> WispersStatus {
    if handle.is_null() || data.is_null() {
        return WispersStatus::NullPointer;
    }

    let wrapper = unsafe { &*handle };
    let data_slice = unsafe { std::slice::from_raw_parts(data, len) };

    match wrapper.0.send(data_slice) {
        Ok(()) => WispersStatus::Success,
        Err(_) => WispersStatus::ConnectionFailed,
    }
}

/// Receive data from a UDP connection.
///
/// The connection handle is NOT consumed.
/// On success, callback receives the data buffer. The buffer is only valid
/// during the callback invocation.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_udp_connection_recv_async(
    handle: *mut WispersUdpConnectionHandle,
    ctx: *mut c_void,
    callback: WispersDataCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    // We need to keep the connection alive while receiving.
    // recv() takes &self, so we borrow via raw pointer. This is safe
    // as long as the caller doesn't free the handle while recv is pending.
    // A safer approach would be to wrap in Arc, but that changes the API.
    let ctx = CallbackContext(ctx);
    let conn_ptr_wrapper = SendableUdpConnPtr(handle);

    runtime::spawn(async move {
        let conn = unsafe { conn_ptr_wrapper.get() };
        let result = conn.recv().await;

        match result {
            Ok(data) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success, data.as_ptr(), data.len());
                }
            }
            Err(_) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::ConnectionFailed, std::ptr::null(), 0);
                }
            }
        }
    });

    WispersStatus::Success
}

/// Close a UDP connection.
///
/// The connection handle is CONSUMED by this call.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_udp_connection_close(handle: *mut WispersUdpConnectionHandle) {
    if handle.is_null() {
        return;
    }

    let wrapper = unsafe { Box::from_raw(handle) };
    wrapper.0.close();
}

//------------------------------------------------------------------------------
// QUIC Connection FFI
//------------------------------------------------------------------------------

/// Opaque handle to a QUIC P2P connection.
pub struct WispersQuicConnectionHandle(pub(crate) QuicConnection);

/// Opaque handle to a QUIC stream.
pub struct WispersQuicStreamHandle(pub(crate) QuicStream);

// Helpers to send raw pointers across threads
struct SendableQuicConnPtr(*mut WispersQuicConnectionHandle);
unsafe impl Send for SendableQuicConnPtr {}

impl SendableQuicConnPtr {
    unsafe fn get(&self) -> &QuicConnection {
        unsafe { &(*self.0).0 }
    }
}

struct SendableQuicStreamPtr(*mut WispersQuicStreamHandle);
unsafe impl Send for SendableQuicStreamPtr {}

impl SendableQuicStreamPtr {
    unsafe fn get(&self) -> &QuicStream {
        unsafe { &(*self.0).0 }
    }
}

/// Callback that receives a QUIC connection handle.
pub type WispersQuicConnectionCallback = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        status: WispersStatus,
        connection: *mut WispersQuicConnectionHandle,
    ),
>;

/// Callback that receives a QUIC stream handle.
pub type WispersQuicStreamCallback = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        status: WispersStatus,
        stream: *mut WispersQuicStreamHandle,
    ),
>;

/// Free a QUIC connection handle.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_quic_connection_free(handle: *mut WispersQuicConnectionHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

/// Free a QUIC stream handle.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_quic_stream_free(handle: *mut WispersQuicStreamHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

/// Connect to a peer node using QUIC transport.
///
/// The activated handle is NOT consumed.
/// On success, callback receives the QUIC connection handle.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_activated_node_connect_quic_async(
    handle: *mut super::handles::WispersActivatedNodeHandle,
    peer_node_number: c_int,
    ctx: *mut c_void,
    callback: WispersQuicConnectionCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let ctx = CallbackContext(ctx);
    let activated_ptr = SendableActivatedPtr(handle);

    runtime::spawn(async move {
        let wrapper = unsafe { activated_ptr.get() };
        let result = match &wrapper.0 {
            ActivatedImpl::InMemory(activated) => activated.connect_quic(peer_node_number).await,
            ActivatedImpl::Foreign(activated) => activated.connect_quic(peer_node_number).await,
        };

        match result {
            Ok(conn) => {
                let h = Box::into_raw(Box::new(WispersQuicConnectionHandle(conn)));
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success, h);
                }
            }
            Err(_) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::ConnectionFailed, std::ptr::null_mut());
                }
            }
        }
    });

    WispersStatus::Success
}

/// Open a new bidirectional stream on a QUIC connection.
///
/// The connection handle is NOT consumed.
/// On success, callback receives the stream handle.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_quic_connection_open_stream_async(
    handle: *mut WispersQuicConnectionHandle,
    ctx: *mut c_void,
    callback: WispersQuicStreamCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let ctx = CallbackContext(ctx);
    let conn_ptr = SendableQuicConnPtr(handle);

    runtime::spawn(async move {
        let conn = unsafe { conn_ptr.get() };
        let result = conn.open_stream().await;

        match result {
            Ok(stream) => {
                let h = Box::into_raw(Box::new(WispersQuicStreamHandle(stream)));
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success, h);
                }
            }
            Err(_) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::ConnectionFailed, std::ptr::null_mut());
                }
            }
        }
    });

    WispersStatus::Success
}

/// Accept an incoming stream from the peer.
///
/// The connection handle is NOT consumed.
/// On success, callback receives the stream handle.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_quic_connection_accept_stream_async(
    handle: *mut WispersQuicConnectionHandle,
    ctx: *mut c_void,
    callback: WispersQuicStreamCallback,
) -> WispersStatus {
    if handle.is_null() {
        return WispersStatus::NullPointer;
    }

    let callback = match callback {
        Some(cb) => cb,
        None => return WispersStatus::MissingCallback,
    };

    let ctx = CallbackContext(ctx);
    let conn_ptr = SendableQuicConnPtr(handle);

    runtime::spawn(async move {
        let conn = unsafe { conn_ptr.get() };
        let result = conn.accept_stream().await;

        match result {
            Ok(stream) => {
                let h = Box::into_raw(Box::new(WispersQuicStreamHandle(stream)));
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success, h);
                }
            }
            Err(_) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::ConnectionFailed, std::ptr::null_mut());
                }
            }
        }
    });

    WispersStatus::Success
}

/// Close a QUIC connection.
///
/// The connection handle is CONSUMED by this call.
/// The callback is invoked when the close operation completes.
#[unsafe(no_mangle)]
pub extern "C" fn wispers_quic_connection_close_async(
    handle: *mut WispersQuicConnectionHandle,
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

    let ctx = CallbackContext(ctx);
    let conn = unsafe { Box::from_raw(handle) };

    runtime::spawn(async move {
        let result = conn.0.close().await;

        match result {
            Ok(()) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::Success);
                }
            }
            Err(_) => {
                unsafe {
                    callback(ctx.ptr(), WispersStatus::ConnectionFailed);
                }
            }
        }
    });

    WispersStatus::Success
}
