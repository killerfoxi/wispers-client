#ifndef WISPERS_CONNECT_H
#define WISPERS_CONNECT_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>

// Status codes returned by the FFI functions.
typedef enum {
    WISPERS_STATUS_SUCCESS = 0,
    WISPERS_STATUS_NULL_POINTER = 1,
    WISPERS_STATUS_INVALID_UTF8 = 2,
    WISPERS_STATUS_STORE_ERROR = 3,
    WISPERS_STATUS_ALREADY_REGISTERED = 4,
    WISPERS_STATUS_NOT_REGISTERED = 5,
    WISPERS_STATUS_UNEXPECTED_STAGE = 6,
    WISPERS_STATUS_NOT_FOUND = 7,
    WISPERS_STATUS_BUFFER_TOO_SMALL = 8,
    WISPERS_STATUS_MISSING_CALLBACK = 9,
    WISPERS_STATUS_INVALID_PAIRING_CODE = 10,
    WISPERS_STATUS_ACTIVATION_FAILED = 11,
    WISPERS_STATUS_HUB_ERROR = 12,
    WISPERS_STATUS_CONNECTION_FAILED = 13,
    WISPERS_STATUS_TIMEOUT = 14,
} WispersStatus;

// Node state stages.
typedef enum {
    WISPERS_STAGE_PENDING = 0,
    WISPERS_STAGE_REGISTERED = 1,
    WISPERS_STAGE_ACTIVATED = 2,
} WispersStage;

// Forward declarations for opaque handles.
typedef struct WispersNodeStorageHandle WispersNodeStorageHandle;
typedef struct WispersPendingNodeHandle WispersPendingNodeHandle;
typedef struct WispersRegisteredNodeHandle WispersRegisteredNodeHandle;
typedef struct WispersActivatedNodeHandle WispersActivatedNodeHandle;

// Host-provided storage callbacks. All functions must be non-null when used.
// The ctx pointer carries all context the host needs, including any namespace
// or isolation information used to keep different node storages separate. The
// library does not manage namespacing.
typedef struct {
    void *ctx;
    WispersStatus (*load_root_key)(void *ctx, uint8_t *out_key, size_t out_key_len);
    WispersStatus (*save_root_key)(void *ctx, const uint8_t *key, size_t key_len);
    WispersStatus (*delete_root_key)(void *ctx);

    // Registration payloads are serialized by Rust (currently using bincode).
    WispersStatus (*load_registration)(void *ctx, uint8_t *buffer, size_t buffer_len, size_t *out_len);
    WispersStatus (*save_registration)(void *ctx, const uint8_t *buffer, size_t buffer_len);
    WispersStatus (*delete_registration)(void *ctx);
} WispersNodeStorageCallbacks;

//------------------------------------------------------------------------------
// Callback types for async operations
//------------------------------------------------------------------------------

// Basic completion callback (no result value).
typedef void (*WispersCallback)(void *ctx, WispersStatus status);

// Callback for restore_or_init, which restores the node at its current stage
// (pending, registered, or activated). `stage` will be set to the appropriate
// enum value and the matching handle will be filled in. Exactly one handle will
// be non-null on success.
typedef void (*WispersInitCallback)(
    void *ctx,
    WispersStatus status,
    WispersStage stage,
    WispersPendingNodeHandle *pending,
    WispersRegisteredNodeHandle *registered,
    WispersActivatedNodeHandle *activated
);

// Callback that receives a registered state handle.
typedef void (*WispersRegisteredCallback)(
    void *ctx,
    WispersStatus status,
    WispersRegisteredNodeHandle *handle
);

// Callback that receives an activated node handle.
typedef void (*WispersActivatedCallback)(
    void *ctx,
    WispersStatus status,
    WispersActivatedNodeHandle *handle
);

//------------------------------------------------------------------------------
// Registration info (returned by read_registration)
//------------------------------------------------------------------------------

// Registration information. Strings are owned and must be freed with wispers_string_free().
typedef struct {
    char *connectivity_group_id;  // Owned, free with wispers_string_free()
    int32_t node_number;
    char *auth_token;             // Owned, free with wispers_string_free()
} WispersRegistrationInfo;

// Free a registration info struct and its strings.
void wispers_registration_info_free(WispersRegistrationInfo *info);

//------------------------------------------------------------------------------
// Storage lifecycle
//------------------------------------------------------------------------------

WispersNodeStorageHandle *wispers_storage_new_in_memory(void);
WispersNodeStorageHandle *wispers_storage_new_with_callbacks(const WispersNodeStorageCallbacks *callbacks);
void wispers_storage_free(WispersNodeStorageHandle *handle);

// Read registration from local storage (sync, no hub contact).
// Returns SUCCESS with out_info populated if registered, NOT_FOUND if not registered.
// Caller must free out_info with wispers_registration_info_free() on success.
WispersStatus wispers_storage_read_registration(
    WispersNodeStorageHandle *handle,
    WispersRegistrationInfo *out_info
);

// Override the hub address (for testing).
WispersStatus wispers_storage_override_hub_addr(
    WispersNodeStorageHandle *handle,
    const char *hub_addr
);

// Restore or initialize node state asynchronously.
// On success, callback receives the stage and exactly one non-null handle.
// The storage handle remains valid and is NOT consumed.
// Returns SUCCESS immediately if the async operation was started.
WispersStatus wispers_storage_restore_or_init_async(
    WispersNodeStorageHandle *handle,
    void *ctx,
    WispersInitCallback callback
);

//------------------------------------------------------------------------------
// Pending state
//------------------------------------------------------------------------------

void wispers_pending_node_free(WispersPendingNodeHandle *handle);

// Logout a pending node (delete local state).
// The pending handle is CONSUMED and must not be used afterward.
// Returns SUCCESS immediately if the async operation was started.
WispersStatus wispers_pending_node_logout_async(
    WispersPendingNodeHandle *handle,
    void *ctx,
    WispersCallback callback
);

// Manual registration completion (for testing or when registration was done out-of-band).
WispersStatus wispers_pending_node_complete_registration(
    WispersPendingNodeHandle *handle,
    const char *connectivity_group_id,
    int node_number,
    const char *auth_token,
    WispersRegisteredNodeHandle **out_registered
);

// Register the pending node with the hub using a registration token.
// On success, callback receives the registered state handle.
// The pending handle is CONSUMED and must not be used afterward.
// Returns SUCCESS immediately if the async operation was started.
WispersStatus wispers_pending_node_register_async(
    WispersPendingNodeHandle *handle,
    const char *token,
    void *ctx,
    WispersRegisteredCallback callback
);


//------------------------------------------------------------------------------
// Registered state
//------------------------------------------------------------------------------

void wispers_registered_node_free(WispersRegisteredNodeHandle *handle);

// Logout a registered node (deregister from hub, then delete local state).
// The registered handle is CONSUMED and must not be used afterward.
// Returns SUCCESS immediately if the async operation was started.
WispersStatus wispers_registered_node_logout_async(
    WispersRegisteredNodeHandle *handle,
    void *ctx,
    WispersCallback callback
);

// TODO: wispers_registered_node_activate_async - Phase 5
// TODO: wispers_registered_node_list_nodes_async - Phase 6

//------------------------------------------------------------------------------
// Activated node
//------------------------------------------------------------------------------

void wispers_activated_node_free(WispersActivatedNodeHandle *handle);

// Logout an activated node (self-revoke from roster, deregister from hub, delete local state).
// The activated handle is CONSUMED and must not be used afterward.
// Returns SUCCESS immediately if the async operation was started.
WispersStatus wispers_activated_node_logout_async(
    WispersActivatedNodeHandle *handle,
    void *ctx,
    WispersCallback callback
);

// TODO: wispers_activated_node_connect_udp_async - Phase 8
// TODO: wispers_activated_node_connect_quic_async - Phase 8

//------------------------------------------------------------------------------
// Utilities
//------------------------------------------------------------------------------

// Free strings allocated by the library.
void wispers_string_free(char *ptr);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // WISPERS_CONNECT_H
