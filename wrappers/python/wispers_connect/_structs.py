"""ctypes Structure subclasses matching C structs in wispers_connect.h."""

from __future__ import annotations

from ctypes import (
    CFUNCTYPE,
    POINTER,
    Structure,
    c_bool,
    c_char_p,
    c_int,
    c_int32,
    c_int64,
    c_size_t,
    c_uint8,
    c_void_p,
)

# ---------------------------------------------------------------------------
# Storage callback function-pointer types
# ---------------------------------------------------------------------------

LoadRootKeyFunc = CFUNCTYPE(c_int, c_void_p, POINTER(c_uint8), c_size_t)
SaveRootKeyFunc = CFUNCTYPE(c_int, c_void_p, POINTER(c_uint8), c_size_t)
DeleteRootKeyFunc = CFUNCTYPE(c_int, c_void_p)

LoadRegistrationFunc = CFUNCTYPE(c_int, c_void_p, POINTER(c_uint8), c_size_t, POINTER(c_size_t))
SaveRegistrationFunc = CFUNCTYPE(c_int, c_void_p, POINTER(c_uint8), c_size_t)
DeleteRegistrationFunc = CFUNCTYPE(c_int, c_void_p)


# ---------------------------------------------------------------------------
# Structures
# ---------------------------------------------------------------------------

class WispersNodeStorageCallbacks(Structure):
    _fields_ = [
        ("ctx", c_void_p),
        ("load_root_key", LoadRootKeyFunc),
        ("save_root_key", SaveRootKeyFunc),
        ("delete_root_key", DeleteRootKeyFunc),
        ("load_registration", LoadRegistrationFunc),
        ("save_registration", SaveRegistrationFunc),
        ("delete_registration", DeleteRegistrationFunc),
    ]


class WispersRegistrationInfo(Structure):
    _fields_ = [
        ("connectivity_group_id", c_char_p),
        ("node_number", c_int32),
        ("auth_token", c_char_p),
        ("attestation_jwt", c_char_p),
    ]


class WispersNode(Structure):
    # Uses c_bool (1 byte) for Rust bool — NOT c_int (4 bytes).
    _fields_ = [
        ("node_number", c_int32),
        ("name", c_char_p),
        ("metadata", c_char_p),
        ("is_self", c_bool),
        ("activation_status", c_int32),
        ("last_seen_at_millis", c_int64),
        ("is_online", c_bool),
    ]


class WispersGroupInfo(Structure):
    _fields_ = [
        ("state", c_int),
        ("nodes", POINTER(WispersNode)),
        ("nodes_count", c_size_t),
    ]
