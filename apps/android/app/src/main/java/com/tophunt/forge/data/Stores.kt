package com.tophunt.forge.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.first
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@Serializable
data class Tokens(val access: String, val refresh: String)

/** Where the session's tokens live. Never logged, never shown, never handed to the WebView. */
interface TokenStore {
    suspend fun read(): Tokens?
    suspend fun write(tokens: Tokens)
    suspend fun clear()
}

/** App settings that aren't secret. */
interface SettingsStore {
    suspend fun server(): String?
    suspend fun setServer(origin: String?)
    suspend fun onboardingDone(): Boolean
    suspend fun setOnboardingDone()
}

class InMemoryTokenStore(private var tokens: Tokens? = null) : TokenStore {
    override suspend fun read() = tokens
    override suspend fun write(tokens: Tokens) { this.tokens = tokens }
    override suspend fun clear() { tokens = null }
}

class InMemorySettingsStore(private var server: String? = null, private var onboarded: Boolean = false) : SettingsStore {
    override suspend fun server() = server
    override suspend fun setServer(origin: String?) { server = origin }
    override suspend fun onboardingDone() = onboarded
    override suspend fun setOnboardingDone() { onboarded = true }
}

/** AES-256-GCM with a fresh random IV per message. Output: base64(iv ‖ ciphertext+tag). */
class Sealer(private val key: () -> SecretKey) {
    fun seal(plain: ByteArray): String {
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, key())
        return java.util.Base64.getEncoder().encodeToString(c.iv + c.doFinal(plain))
    }

    /** `null` if the data was tampered with, sealed with another key, or isn't ours. */
    fun open(sealed: String): ByteArray? = runCatching {
        val all = java.util.Base64.getDecoder().decode(sealed)
        require(all.size > IV_LEN)
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, all, 0, IV_LEN))
        c.doFinal(all, IV_LEN, all.size - IV_LEN)
    }.getOrNull()

    companion object {
        private const val IV_LEN = 12
        private const val ALIAS = "forge.tokens.v1"

        /** A key that never leaves the Android Keystore (hardware-backed where the device has it). */
        fun keystoreKey(): SecretKey {
            val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
            val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
            gen.init(
                KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .build(),
            )
            return gen.generateKey()
        }
    }
}

private val Context.store: DataStore<Preferences> by preferencesDataStore(name = "forge")
private val SEALED_TOKENS = stringPreferencesKey("tokens.sealed")
private val SERVER = stringPreferencesKey("server")
private val ONBOARDED = booleanPreferencesKey("onboarded")

/** Tokens in DataStore, sealed with a Keystore key. Unreadable data counts as signed out. */
class SealedTokenStore(context: Context, private val sealer: Sealer = Sealer(Sealer::keystoreKey)) : TokenStore {
    private val ds = context.store
    override suspend fun read(): Tokens? {
        val sealed = ds.data.first()[SEALED_TOKENS] ?: return null
        val plain = sealer.open(sealed) ?: return null
        return runCatching { Json.decodeFromString<Tokens>(plain.decodeToString()) }.getOrNull()
    }
    override suspend fun write(tokens: Tokens) {
        val sealed = sealer.seal(Json.encodeToString(Tokens.serializer(), tokens).encodeToByteArray())
        ds.edit { it[SEALED_TOKENS] = sealed }
    }
    override suspend fun clear() {
        ds.edit { it.remove(SEALED_TOKENS) }
    }
}

class DataStoreSettings(context: Context) : SettingsStore {
    private val ds = context.store
    override suspend fun server() = ds.data.first()[SERVER]
    override suspend fun setServer(origin: String?) {
        ds.edit { if (origin == null) it.remove(SERVER) else it[SERVER] = origin }
    }
    override suspend fun onboardingDone() = ds.data.first()[ONBOARDED] ?: false
    override suspend fun setOnboardingDone() {
        ds.edit { it[ONBOARDED] = true }
    }
}
