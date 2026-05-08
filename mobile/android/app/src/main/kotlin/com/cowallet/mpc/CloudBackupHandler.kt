package com.cowallet.mpc

import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.annotation.RequiresApi
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.security.KeyStore
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.spec.GCMParameterSpec

class CloudBackupHandler(private val context: Context) : MethodChannel.MethodCallHandler {
  companion object {
    private const val CHANNEL = "com.cowallet/cloud_backup"
    private const val PREFS_NAME = "cowallet_cloud_backup"
    private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
    private const val KEY_ALIAS = "com.cowallet.backup.encryption"
    private const val CIPHER_TRANSFORMATION = "AES/GCM/NoPadding"
    private const val GCM_TAG_LENGTH_BITS = 128
    private const val IV_LENGTH_BYTES = 12

    fun setup(flutterEngine: FlutterEngine, context: Context) {
      val channel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL)
      channel.setMethodCallHandler(CloudBackupHandler(context))
    }
  }

  override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
    when (call.method) {
      "isAvailable" -> result.success(Build.VERSION.SDK_INT >= Build.VERSION_CODES.M)
      "store" -> store(call, result)
      "retrieve" -> retrieve(call, result)
      "delete" -> delete(call, result)
      else -> result.notImplemented()
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun store(call: MethodCall, result: MethodChannel.Result) {
    val key = call.argument<String>("key")
    val data = call.argument<String>("data")

    if (key == null || data == null) {
      result.error("INVALID_ARGS", "key and data are required", null)
      return
    }

    try {
      ensureKeyExists()
      val encrypted = encrypt(data.toByteArray(Charsets.UTF_8))
      getPrefs().edit().putString(key, encrypted).apply()
      result.success(null)
    } catch (e: Exception) {
      result.error("STORE_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun retrieve(call: MethodCall, result: MethodChannel.Result) {
    val key = call.argument<String>("key")

    if (key == null) {
      result.error("INVALID_ARGS", "key is required", null)
      return
    }

    try {
      val encrypted = getPrefs().getString(key, null)
      if (encrypted == null) {
        result.success(null)
        return
      }
      val decrypted = decrypt(encrypted)
      result.success(String(decrypted, Charsets.UTF_8))
    } catch (e: Exception) {
      result.error("RETRIEVE_FAILED", e.message, null)
    }
  }

  private fun delete(call: MethodCall, result: MethodChannel.Result) {
    val key = call.argument<String>("key")

    if (key == null) {
      result.error("INVALID_ARGS", "key is required", null)
      return
    }

    try {
      getPrefs().edit().remove(key).apply()
      result.success(null)
    } catch (e: Exception) {
      result.error("DELETE_FAILED", e.message, null)
    }
  }

  private fun getPrefs(): SharedPreferences {
    return context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun ensureKeyExists() {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)

    if (!keyStore.containsAlias(KEY_ALIAS)) {
      val spec = KeyGenParameterSpec.Builder(
        KEY_ALIAS,
        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
      )
        .setKeySize(256)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .build()

      val keyGenerator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
      keyGenerator.init(spec)
      keyGenerator.generateKey()
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun encrypt(plaintext: ByteArray): String {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)
    val secretKey = keyStore.getKey(KEY_ALIAS, null)

    val cipher = Cipher.getInstance(CIPHER_TRANSFORMATION)
    cipher.init(Cipher.ENCRYPT_MODE, secretKey)

    val iv = cipher.iv
    val ciphertext = cipher.doFinal(plaintext)

    val combined = ByteArray(iv.size + ciphertext.size)
    System.arraycopy(iv, 0, combined, 0, iv.size)
    System.arraycopy(ciphertext, 0, combined, iv.size, ciphertext.size)

    return Base64.getEncoder().encodeToString(combined)
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun decrypt(encryptedData: String): ByteArray {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)
    val secretKey = keyStore.getKey(KEY_ALIAS, null)

    val combined = Base64.getDecoder().decode(encryptedData)
    val iv = ByteArray(IV_LENGTH_BYTES)
    val ciphertext = ByteArray(combined.size - IV_LENGTH_BYTES)

    System.arraycopy(combined, 0, iv, 0, IV_LENGTH_BYTES)
    System.arraycopy(combined, IV_LENGTH_BYTES, ciphertext, 0, ciphertext.size)

    val cipher = Cipher.getInstance(CIPHER_TRANSFORMATION)
    cipher.init(Cipher.DECRYPT_MODE, secretKey, GCMParameterSpec(GCM_TAG_LENGTH_BITS, iv))

    return cipher.doFinal(ciphertext)
  }
}
