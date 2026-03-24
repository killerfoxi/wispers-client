"""NodeStorage handle + StorageCallbacks protocol + FileStorage."""

from __future__ import annotations

import ctypes
from pathlib import Path
from typing import Any, Protocol

from ._bridge import INIT_CB, call_async
from ._handle import Handle
from ._structs import WispersNodeStorageCallbacks, WispersRegistrationInfo
from .exceptions import raise_for_status
from .types import NodeState, RegistrationInfo, Status


class StorageCallbacks(Protocol):
    """Host-provided storage callbacks (6 methods)."""

    def load_root_key(self) -> bytes | None: ...
    def save_root_key(self, key: bytes) -> None: ...
    def delete_root_key(self) -> None: ...
    def load_registration(self) -> bytes | None: ...
    def save_registration(self, data: bytes) -> None: ...
    def delete_registration(self) -> None: ...


class FileStorage:
    """Default StorageCallbacks implementation backed by files on disk."""

    def __init__(self, directory: str | Path) -> None:
        self._dir = Path(directory)
        self._dir.mkdir(parents=True, exist_ok=True)

    def _key_path(self) -> Path:
        return self._dir / "root_key.bin"

    def _reg_path(self) -> Path:
        return self._dir / "registration.pb"

    def load_root_key(self) -> bytes | None:
        p = self._key_path()
        return p.read_bytes() if p.exists() else None

    def save_root_key(self, key: bytes) -> None:
        self._key_path().write_bytes(key)

    def delete_root_key(self) -> None:
        p = self._key_path()
        if p.exists():
            p.unlink()

    def load_registration(self) -> bytes | None:
        p = self._reg_path()
        return p.read_bytes() if p.exists() else None

    def save_registration(self, data: bytes) -> None:
        self._reg_path().write_bytes(data)

    def delete_registration(self) -> None:
        p = self._reg_path()
        if p.exists():
            p.unlink()


def _make_c_callbacks(
    py_cb: StorageCallbacks,
) -> tuple[WispersNodeStorageCallbacks, list[Any]]:
    """Build C callback struct from a Python StorageCallbacks, returning prevent-GC refs."""
    from ._structs import (
        DeleteRegistrationFunc,
        DeleteRootKeyFunc,
        LoadRegistrationFunc,
        LoadRootKeyFunc,
        SaveRegistrationFunc,
        SaveRootKeyFunc,
    )

    prevent_gc: list[Any] = []

    @LoadRootKeyFunc  # type: ignore[untyped-decorator]
    def load_root_key(_ctx: int, out_key: Any, out_key_len: int) -> int:
        try:
            data = py_cb.load_root_key()
        except Exception:
            return Status.STORE_ERROR
        if data is None:
            return Status.NOT_FOUND
        if len(data) > out_key_len:
            return Status.BUFFER_TOO_SMALL
        ctypes.memmove(out_key, data, len(data))
        return Status.SUCCESS

    @SaveRootKeyFunc  # type: ignore[untyped-decorator]
    def save_root_key(_ctx: int, key: Any, key_len: int) -> int:
        try:
            py_cb.save_root_key(ctypes.string_at(key, key_len))
        except Exception:
            return Status.STORE_ERROR
        return Status.SUCCESS

    @DeleteRootKeyFunc  # type: ignore[untyped-decorator]
    def delete_root_key(_ctx: int) -> int:
        try:
            py_cb.delete_root_key()
        except Exception:
            return Status.STORE_ERROR
        return Status.SUCCESS

    @LoadRegistrationFunc  # type: ignore[untyped-decorator]
    def load_registration(_ctx: int, buffer: Any, buffer_len: int, out_len: Any) -> int:
        try:
            data = py_cb.load_registration()
        except Exception:
            return Status.STORE_ERROR
        if data is None:
            return Status.NOT_FOUND
        # Write actual length.
        out_len[0] = len(data)
        if len(data) > buffer_len:
            return Status.BUFFER_TOO_SMALL
        ctypes.memmove(buffer, data, len(data))
        return Status.SUCCESS

    @SaveRegistrationFunc  # type: ignore[untyped-decorator]
    def save_registration(_ctx: int, buffer: Any, buffer_len: int) -> int:
        try:
            py_cb.save_registration(ctypes.string_at(buffer, buffer_len))
        except Exception:
            return Status.STORE_ERROR
        return Status.SUCCESS

    @DeleteRegistrationFunc  # type: ignore[untyped-decorator]
    def delete_registration(_ctx: int) -> int:
        try:
            py_cb.delete_registration()
        except Exception:
            return Status.STORE_ERROR
        return Status.SUCCESS

    prevent_gc.extend([
        load_root_key, save_root_key, delete_root_key,
        load_registration, save_registration, delete_registration,
    ])

    cbs = WispersNodeStorageCallbacks(
        ctx=None,
        load_root_key=load_root_key,
        save_root_key=save_root_key,
        delete_root_key=delete_root_key,
        load_registration=load_registration,
        save_registration=save_registration,
        delete_registration=delete_registration,
    )
    return cbs, prevent_gc


class NodeStorage(Handle):
    """Wraps a WispersNodeStorageHandle."""

    def __init__(self, ptr: Any, prevent_gc: list[Any] | None = None) -> None:
        super().__init__(ptr)
        # prevent_gc keeps CFUNCTYPE instances alive while native code holds pointers.
        self._prevent_gc = prevent_gc or []

    @staticmethod
    def in_memory() -> NodeStorage:
        """Create an in-memory storage (for testing)."""
        from ._library import get_lib
        ptr = get_lib().wispers_storage_new_in_memory()
        return NodeStorage(ptr)

    @staticmethod
    def with_callbacks(cb: StorageCallbacks) -> NodeStorage:
        """Create storage backed by host-provided callbacks."""
        from ._library import get_lib
        c_cbs, prevent_gc = _make_c_callbacks(cb)
        # Keep the struct itself alive too.
        prevent_gc.append(c_cbs)
        ptr = get_lib().wispers_storage_new_with_callbacks(ctypes.byref(c_cbs))
        return NodeStorage(ptr, prevent_gc)

    @staticmethod
    def with_file_storage(directory: str | Path) -> NodeStorage:
        """Create storage backed by files in *directory*."""
        fs = FileStorage(directory)
        storage = NodeStorage.with_callbacks(fs)
        storage._prevent_gc.append(fs)
        return storage

    def read_registration(self) -> RegistrationInfo:
        """Read registration from local storage (sync, no hub contact).

        Raises NotFoundError if not registered.
        """
        from ._library import get_lib
        lib = get_lib()
        ptr = self._require_open()
        info = WispersRegistrationInfo()
        status = lib.wispers_storage_read_registration(ptr, ctypes.byref(info))
        raise_for_status(status)
        result = RegistrationInfo(
            connectivity_group_id=info.connectivity_group_id.decode("utf-8") if info.connectivity_group_id else "",
            node_number=info.node_number,
            auth_token=info.auth_token.decode("utf-8") if info.auth_token else "",
            attestation_jwt=info.attestation_jwt.decode("utf-8") if info.attestation_jwt else "",
        )
        lib.wispers_registration_info_free(ctypes.byref(info))
        return result

    def override_hub_addr(self, addr: str) -> None:
        """Override the hub address (for testing/staging)."""
        from ._library import get_lib
        ptr = self._require_open()
        status = get_lib().wispers_storage_override_hub_addr(ptr, addr.encode("utf-8"))
        raise_for_status(status)

    def delete_state(self) -> None:
        """Delete all persisted state. Used for logout when the node can't be restored."""
        from ._library import get_lib
        ptr = self._require_open()
        status = get_lib().wispers_storage_delete_state(ptr)
        raise_for_status(status)

    def restore_or_init(self) -> tuple[Any, NodeState]:
        """Restore or initialize node state. Returns (Node, NodeState).

        The NodeStorage remains valid after this call.
        """
        from ._library import get_lib
        from .node import Node
        ptr = self._require_open()
        result = call_async(
            get_lib().wispers_storage_restore_or_init_async, ptr, cb=INIT_CB,
        )
        node_ptr, state = result
        return Node(node_ptr), state

    def _do_close(self, ptr: Any) -> None:
        from ._library import get_lib
        get_lib().wispers_storage_free(ptr)
        self._prevent_gc.clear()
