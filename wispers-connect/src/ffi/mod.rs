mod callbacks;
mod handles;
mod helpers;
mod manager;
mod nodes;
mod p2p;
pub(crate) mod runtime;
mod serving;

pub use callbacks::{WispersCallback, WispersInitCallback, WispersNodeListCallback, WispersNodeState};
pub use handles::{WispersNodeHandle, WispersNodeStorageHandle};
pub use helpers::{
    wispers_node_list_free, wispers_registration_info_free, wispers_string_free, WispersNode,
    WispersNodeList, WispersRegistrationInfo,
};
pub use manager::{
    wispers_storage_free, wispers_storage_new_in_memory, wispers_storage_new_with_callbacks,
    wispers_storage_override_hub_addr, wispers_storage_read_registration,
    wispers_storage_restore_or_init_async,
};
pub use nodes::{
    wispers_node_activate_async, wispers_node_free, wispers_node_list_nodes_async,
    wispers_node_logout_async, wispers_node_register_async, wispers_node_state,
};
pub use serving::{
    wispers_incoming_accept_quic_async, wispers_incoming_accept_udp_async,
    wispers_incoming_connections_free, wispers_node_start_serving_async,
    wispers_serving_handle_free, wispers_serving_handle_generate_pairing_code_async,
    wispers_serving_handle_shutdown_async, wispers_serving_session_free,
    wispers_serving_session_run_async, WispersIncomingConnections, WispersPairingCodeCallback,
    WispersServingHandle, WispersServingSession, WispersStartServingCallback,
};
pub use p2p::{
    wispers_node_connect_quic_async, wispers_node_connect_udp_async,
    wispers_quic_connection_accept_stream_async, wispers_quic_connection_close_async,
    wispers_quic_connection_free, wispers_quic_connection_open_stream_async,
    wispers_quic_stream_finish_async, wispers_quic_stream_free, wispers_quic_stream_read_async,
    wispers_quic_stream_shutdown_async, wispers_quic_stream_write_async,
    wispers_udp_connection_close, wispers_udp_connection_free, wispers_udp_connection_recv_async,
    wispers_udp_connection_send, WispersDataCallback, WispersQuicConnectionCallback,
    WispersQuicConnectionHandle, WispersQuicStreamCallback, WispersQuicStreamHandle,
    WispersUdpConnectionCallback, WispersUdpConnectionHandle,
};

pub use crate::storage::foreign::WispersNodeStorageCallbacks;
