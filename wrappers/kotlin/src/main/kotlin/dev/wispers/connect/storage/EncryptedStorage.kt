package dev.wispers.connect.storage

import android.content.Context
import android.content.SharedPreferences
import android.util.Base64
import android.util.Log
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * Secure storage implementation using Android's EncryptedSharedPreferences.
 *
 * This is the recommended storage for production use. Data is encrypted using:
 * - AES256-GCM for values
 * - AES256-SIV for keys (deterministic encryption for key lookup)
 *
 * The master key is stored in Android Keystore, protected by hardware if available.
 *
 * Usage:
 * ```kotlin
 * val storage = WispersConnect.createStorage(context)
 * ```
 */
internal class EncryptedStorage private constructor(
    private val prefs: SharedPreferences
) : NodeStorageCallbacks {

    override fun loadRootKey(): ByteArray? {
        val encoded = prefs.getString(KEY_ROOT_KEY, null)
        Log.d(TAG, "loadRootKey: ${if (encoded != null) "${encoded.length} chars" else "null"}")
        return encoded?.let { Base64.decode(it, Base64.NO_WRAP) }
    }

    override fun saveRootKey(key: ByteArray) {
        val encoded = Base64.encodeToString(key, Base64.NO_WRAP)
        val ok = prefs.edit()
            .putString(KEY_ROOT_KEY, encoded)
            .commit()
        Log.d(TAG, "saveRootKey: ${key.size} bytes, commit=$ok")
    }

    override fun deleteRootKey() {
        val ok = prefs.edit()
            .remove(KEY_ROOT_KEY)
            .commit()
        Log.d(TAG, "deleteRootKey: commit=$ok")
    }

    override fun loadRegistration(): ByteArray? {
        val encoded = prefs.getString(KEY_REGISTRATION, null)
        Log.d(TAG, "loadRegistration: ${if (encoded != null) "${encoded.length} chars" else "null"}")
        return encoded?.let { Base64.decode(it, Base64.NO_WRAP) }
    }

    override fun saveRegistration(data: ByteArray) {
        val encoded = Base64.encodeToString(data, Base64.NO_WRAP)
        val ok = prefs.edit()
            .putString(KEY_REGISTRATION, encoded)
            .commit()
        Log.d(TAG, "saveRegistration: ${data.size} bytes, commit=$ok")
    }

    override fun deleteRegistration() {
        val ok = prefs.edit()
            .remove(KEY_REGISTRATION)
            .commit()
        Log.d(TAG, "deleteRegistration: commit=$ok")
    }

    companion object {
        private const val TAG = "WispersStorage"
        private const val PREFS_FILE_NAME = "wispers_connect_storage"
        private const val KEY_ROOT_KEY = "root_key"
        private const val KEY_REGISTRATION = "registration"

        /**
         * Create an encrypted storage instance.
         *
         * @param context Application or activity context
         * @return A new EncryptedStorage instance
         */
        fun create(context: Context): EncryptedStorage {
            val masterKey = MasterKey.Builder(context)
                .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                .build()

            val prefs = EncryptedSharedPreferences.create(
                context,
                PREFS_FILE_NAME,
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
            )

            return EncryptedStorage(prefs)
        }
    }
}
