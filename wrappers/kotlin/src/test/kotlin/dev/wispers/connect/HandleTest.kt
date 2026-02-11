package dev.wispers.connect

import com.sun.jna.Pointer
import dev.wispers.connect.handles.Handle
import org.junit.Assert.*
import org.junit.Test

class HandleTest {

    @Test
    fun `requireOpen returns pointer when not closed`() {
        val handle = TestHandle(Pointer.createConstant(123))

        val ptr = handle.testRequireOpen()

        assertEquals(123L, Pointer.nativeValue(ptr))
    }

    @Test
    fun `requireOpen throws when closed`() {
        val handle = TestHandle(Pointer.createConstant(123))
        handle.close()

        assertThrows(IllegalStateException::class.java) {
            handle.testRequireOpen()
        }
    }

    @Test
    fun `consume returns pointer and marks as closed`() {
        val handle = TestHandle(Pointer.createConstant(456))

        val ptr = handle.testConsume()

        assertEquals(456L, Pointer.nativeValue(ptr))
        assertTrue(handle.isClosed)
    }

    @Test
    fun `consume returns null when already consumed`() {
        val handle = TestHandle(Pointer.createConstant(789))
        handle.testConsume()

        val secondConsume = handle.testConsume()

        assertNull(secondConsume)
    }

    @Test
    fun `close is idempotent`() {
        val handle = TestHandle(Pointer.createConstant(111))

        handle.close()
        handle.close()
        handle.close()

        assertTrue(handle.isClosed)
        assertEquals(1, handle.closeCount)
    }

    @Test
    fun `close calls doClose with pointer`() {
        val ptr = Pointer.createConstant(222)
        val handle = TestHandle(ptr)

        handle.close()

        assertEquals(222L, Pointer.nativeValue(handle.lastClosedPointer))
    }

    @Test
    fun `isClosed returns false initially`() {
        val handle = TestHandle(Pointer.createConstant(333))

        assertFalse(handle.isClosed)
    }

    @Test
    fun `isClosed returns true after close`() {
        val handle = TestHandle(Pointer.createConstant(444))
        handle.close()

        assertTrue(handle.isClosed)
    }

    /**
     * Test implementation of Handle that exposes protected methods.
     */
    private class TestHandle(pointer: Pointer) : Handle(pointer) {
        var closeCount = 0
        var lastClosedPointer: Pointer? = null

        fun testRequireOpen(): Pointer = requireOpen()
        fun testConsume(): Pointer? = consume()

        override fun doClose(pointer: Pointer) {
            closeCount++
            lastClosedPointer = pointer
        }
    }

    private inline fun <reified T : Throwable> assertThrows(
        expectedType: Class<T>,
        executable: () -> Unit
    ): T {
        try {
            executable()
            fail("Expected ${expectedType.simpleName} to be thrown")
            throw AssertionError("Unreachable")
        } catch (e: Throwable) {
            if (expectedType.isInstance(e)) {
                @Suppress("UNCHECKED_CAST")
                return e as T
            }
            throw e
        }
    }
}
