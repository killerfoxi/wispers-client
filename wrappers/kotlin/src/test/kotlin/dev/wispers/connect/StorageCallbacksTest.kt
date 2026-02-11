package dev.wispers.connect

import dev.wispers.connect.storage.NodeStorageCallbacks
import dev.wispers.connect.storage.toNativeCallbacks
import dev.wispers.connect.types.WispersStatus
import org.junit.Assert.*
import org.junit.Test

class StorageCallbacksTest {

    @Test
    fun `toNativeCallbacks creates all callback functions`() {
        val callbacks = TestStorageCallbacks()
        val native = callbacks.toNativeCallbacks()

        assertNotNull(native.loadRootKey)
        assertNotNull(native.saveRootKey)
        assertNotNull(native.deleteRootKey)
        assertNotNull(native.loadRegistration)
        assertNotNull(native.saveRegistration)
        assertNotNull(native.deleteRegistration)
    }

    @Test
    fun `loadRootKey returns NOT_FOUND when no key stored`() {
        val callbacks = TestStorageCallbacks()
        val native = callbacks.toNativeCallbacks()

        val result = native.loadRootKey!!.invoke(null, null, 32)
        assertEquals(WispersStatus.NOT_FOUND.code, result)
    }

    @Test
    fun `saveRootKey and loadRootKey round-trip`() {
        val callbacks = TestStorageCallbacks()
        val testKey = ByteArray(32) { it.toByte() }
        callbacks.rootKey = testKey

        val native = callbacks.toNativeCallbacks()

        // Verify we can load the key back
        val loadedKey = callbacks.loadRootKey()
        assertNotNull(loadedKey)
        assertArrayEquals(testKey, loadedKey)
    }

    @Test
    fun `deleteRootKey clears stored key`() {
        val callbacks = TestStorageCallbacks()
        callbacks.rootKey = ByteArray(32) { 0x42 }

        callbacks.deleteRootKey()

        assertNull(callbacks.loadRootKey())
    }

    @Test
    fun `loadRegistration returns NOT_FOUND when no registration stored`() {
        val callbacks = TestStorageCallbacks()
        val native = callbacks.toNativeCallbacks()

        val result = native.loadRegistration!!.invoke(null, null, 1024, null)
        assertEquals(WispersStatus.NOT_FOUND.code, result)
    }

    @Test
    fun `saveRegistration and loadRegistration round-trip`() {
        val callbacks = TestStorageCallbacks()
        val testData = "test registration data".toByteArray()
        callbacks.registration = testData

        val loaded = callbacks.loadRegistration()
        assertNotNull(loaded)
        assertArrayEquals(testData, loaded)
    }

    @Test
    fun `deleteRegistration clears stored registration`() {
        val callbacks = TestStorageCallbacks()
        callbacks.registration = "test".toByteArray()

        callbacks.deleteRegistration()

        assertNull(callbacks.loadRegistration())
    }

    /**
     * Test implementation of NodeStorageCallbacks using in-memory storage.
     */
    private class TestStorageCallbacks : NodeStorageCallbacks {
        var rootKey: ByteArray? = null
        var registration: ByteArray? = null

        override fun loadRootKey(): ByteArray? = rootKey?.copyOf()

        override fun saveRootKey(key: ByteArray) {
            rootKey = key.copyOf()
        }

        override fun deleteRootKey() {
            rootKey = null
        }

        override fun loadRegistration(): ByteArray? = registration?.copyOf()

        override fun saveRegistration(data: ByteArray) {
            registration = data.copyOf()
        }

        override fun deleteRegistration() {
            registration = null
        }
    }
}
