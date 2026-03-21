"""QuicConnection + QuicStream handles."""

from __future__ import annotations

import ctypes
from typing import Any

from ._bridge import BASIC_CB, DATA_CB, QUIC_STREAM_CB, call_async
from ._handle import Handle


class QuicStream(Handle):
    """Wraps a WispersQuicStreamHandle."""

    def write(self, data: bytes) -> None:
        """Write data to the QUIC stream."""
        from ._library import get_lib
        ptr = self._require_open()
        buf = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)
        call_async(
            get_lib().wispers_quic_stream_write_async, ptr, buf, len(data), cb=BASIC_CB,
        )

    def read(self, max_len: int = 65536) -> bytes:
        """Read up to max_len bytes from the QUIC stream."""
        from ._library import get_lib
        ptr = self._require_open()
        result: bytes = call_async(
            get_lib().wispers_quic_stream_read_async, ptr, max_len, cb=DATA_CB,
        )
        return result

    def finish(self) -> None:
        """Close the write side (send FIN). Can still read after this."""
        from ._library import get_lib
        ptr = self._require_open()
        call_async(get_lib().wispers_quic_stream_finish_async, ptr, cb=BASIC_CB)

    def shutdown(self) -> None:
        """Shutdown both sides of the stream."""
        from ._library import get_lib
        ptr = self._require_open()
        call_async(get_lib().wispers_quic_stream_shutdown_async, ptr, cb=BASIC_CB)

    def _do_close(self, ptr: Any) -> None:
        from ._library import get_lib
        get_lib().wispers_quic_stream_free(ptr)


class QuicConnection(Handle):
    """Wraps a WispersQuicConnectionHandle."""

    def open_stream(self) -> QuicStream:
        """Open a new bidirectional QUIC stream."""
        from ._library import get_lib
        ptr = self._require_open()
        stream_ptr = call_async(
            get_lib().wispers_quic_connection_open_stream_async, ptr, cb=QUIC_STREAM_CB,
        )
        return QuicStream(stream_ptr)

    def accept_stream(self) -> QuicStream:
        """Accept an incoming QUIC stream from the peer."""
        from ._library import get_lib
        ptr = self._require_open()
        stream_ptr = call_async(
            get_lib().wispers_quic_connection_accept_stream_async, ptr, cb=QUIC_STREAM_CB,
        )
        return QuicStream(stream_ptr)

    def _do_close(self, ptr: Any) -> None:
        from ._library import get_lib
        lib = get_lib()
        # close_async CONSUMES the handle on SUCCESS.  Only call _free if
        # the initial status indicates the async op was never started.
        from ._bridge import BASIC_CB as _cb, _new_pending, _lock, _pending
        call_id, call = _new_pending()
        ctx = ctypes.c_void_p(call_id)
        status = lib.wispers_quic_connection_close_async(ptr, ctx, _cb)
        if status != 0:
            with _lock:
                _pending.pop(call_id, None)
            # Async op not started — handle not consumed, free it.
            lib.wispers_quic_connection_free(ptr)
            return
        # Handle was consumed. Wait for completion, ignore callback errors.
        call.event.wait()
