package com.cowallet.mpc

import android.content.Context
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

class MpcKeystoreHandler(private val context: Context) : MethodChannel.MethodCallHandler {
  companion object {
    private const val CHANNEL = "com.cowallet.mpc/keystore"
    private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
    private const val CIPHER_TRANSFORMATION = "AES/GCM/NoPadding"
    private const val KEY_ALIAS = "com.cowallet.storage.master"
    private const val GCM_TAG_LENGTH_BITS = 128
    private const val IV_LENGTH_BYTES = 12

    fun setup(flutterEngine: FlutterEngine, context: Context) {
      val channel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL)
      channel.setMethodCallHandler(MpcKeystoreHandler(context))
    }
  }

  override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
    when (call.method) {
      "storeSecret" -> {
        val key = call.argument<String>("key")
        val value = call.argument<String>("value")
        if (key != null && value != null) {
          storeSecret(key, value, result)
        } else {
          result.error("INVALID_ARGS", "key and value are required", null)
        }
      }
      "getSecret" -> {
        val key = call.argument<String>("key")
        if (key != null) {
          getSecret(key, result)
        } else {
          result.error("INVALID_ARGS", "key is required", null)
        }
      }
      "deleteSecret" -> {
        val key = call.argument<String>("key")
        if (key != null) {
          deleteSecret(key, result)
        } else {
          result.error("INVALID_ARGS", "key is required", null)
        }
      }
      "storeEncryptedShard" -> {
        val data = call.argument<ByteArray>("data")
        if (data != null) {
          storeEncryptedShard(data, result)
        } else {
          result.error("INVALID_ARGS", "data is required", null)
        }
      }
      "loadEncryptedShard" -> {
        loadEncryptedShard(result)
      }
      else -> result.notImplemented()
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun storeSecret(key: String, value: String, result: MethodChannel.Result) {
    try {
      ensureMasterKeyExists()

      val encryptedData = encryptData(value.toByteArray(Charsets.UTF_8))

      val sharedPref = context.getSharedPreferences("cowallet_secure_storage", Context.MODE_PRIVATE)
      sharedPref.edit().putString(key, encryptedData).apply()

      result.success(null)
    } catch (e: Exception) {
      result.error("STORE_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun getSecret(key: String, result: MethodChannel.Result) {
    try {
      val sharedPref = context.getSharedPreferences("cowallet_secure_storage", Context.MODE_PRIVATE)
      val encryptedData = sharedPref.getString(key, null)

      if (encryptedData == null) {
        result.success(null)
        return
      }

      val decryptedData = decryptData(encryptedData)
      val value = String(decryptedData, Charsets.UTF_8)

      result.success(value)
    } catch (e: Exception) {
      result.error("GET_FAILED", e.message, null)
    }
  }

  private fun deleteSecret(key: String, result: MethodChannel.Result) {
    try {
      val sharedPref = context.getSharedPreferences("cowallet_secure_storage", Context.MODE_PRIVATE)
      sharedPref.edit().remove(key).apply()

      result.success(null)
    } catch (e: Exception) {
      result.error("DELETE_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun ensureMasterKeyExists() {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)

    if (!keyStore.containsAlias(KEY_ALIAS)) {
      val keyGenSpec = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
          .setKeySize(256)
          .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
          .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
          .setIsStrongBoxBacked(true)
          .build()
      } else {
        KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
          .setKeySize(256)
          .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
          .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
          .build()
      }

      val keyGenerator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
      keyGenerator.init(keyGenSpec)
      keyGenerator.generateKey()
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun encryptData(plaintext: ByteArray): String {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)

    val secretKey = keyStore.getKey(KEY_ALIAS, null)
      ?: throw Exception("Master key not found")

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
  private fun decryptData(encryptedData: String): ByteArray {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)

    val secretKey = keyStore.getKey(KEY_ALIAS, null)
      ?: throw Exception("Master key not found")

    val combined = Base64.getDecoder().decode(encryptedData)

    val iv = ByteArray(IV_LENGTH_BYTES)
    val ciphertext = ByteArray(combined.size - IV_LENGTH_BYTES)

    System.arraycopy(combined, 0, iv, 0, IV_LENGTH_BYTES)
    System.arraycopy(combined, IV_LENGTH_BYTES, ciphertext, 0, ciphertext.size)

    val cipher = Cipher.getInstance(CIPHER_TRANSFORMATION)
    cipher.init(Cipher.DECRYPT_MODE, secretKey, GCMParameterSpec(GCM_TAG_LENGTH_BITS, iv))

    return cipher.doFinal(ciphertext)
  }

  // MARK: - Hardware-Backed Shard Encryption

  @RequiresApi(Build.VERSION_CODES.M)
  private fun storeEncryptedShard(shardData: ByteArray, result: MethodChannel.Result) {
    try {
      // Ensure hardware-backed encryption key exists
      ensureShardEncryptionKeyExists()

      // Encrypt the shard data
      val encryptedData = encryptShardData(shardData)

      // Store encrypted data in SharedPreferences
      val sharedPref = context.getSharedPreferences("cowallet_secure_storage", Context.MODE_PRIVATE)
      sharedPref.edit().putString("device-shard-encrypted", encryptedData).apply()

      result.success(null)
    } catch (e: Exception) {
      result.error("ENCRYPTION_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun loadEncryptedShard(result: MethodChannel.Result) {
    try {
      // Retrieve encrypted data from SharedPreferences
      val sharedPref = context.getSharedPreferences("cowallet_secure_storage", Context.MODE_PRIVATE)
      val encryptedData = sharedPref.getString("device-shard-encrypted", null)

      if (encryptedData == null) {
        result.success(null) // No shard stored
        return
      }

      // Decrypt the shard data
      val decryptedData = decryptShardData(encryptedData)

      // Return as byte array
      result.success(decryptedData)
    } catch (e: Exception) {
      result.error("DECRYPTION_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun ensureShardEncryptionKeyExists() {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)

    val shardKeyAlias = "com.cowallet.shard.encryption"

    if (!keyStore.containsAlias(shardKeyAlias)) {
      val keyGenSpec = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        KeyGenParameterSpec.Builder(shardKeyAlias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
          .setKeySize(256)
          .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
          .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
          .setIsStrongBoxBacked(true) // Use StrongBox if available
          .setUserAuthenticationRequired(false) // Don't require auth for every encryption
          .setRandomizedEncryptionRequired(true)
          .build()
      } else {
        KeyGenParameterSpec.Builder(shardKeyAlias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
          .setKeySize(256)
          .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
          .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
          .setRandomizedEncryptionRequired(true)
          .build()
      }

      val keyGenerator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
      keyGenerator.init(keyGenSpec)
      keyGenerator.generateKey()
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun encryptShardData(plaintext: ByteArray): String {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)

    val shardKeyAlias = "com.cowallet.shard.encryption"
    val secretKey = keyStore.getKey(shardKeyAlias, null)
      ?: throw Exception("Shard encryption key not found")

    val cipher = Cipher.getInstance(CIPHER_TRANSFORMATION)
    cipher.init(Cipher.ENCRYPT_MODE, secretKey)

    val iv = cipher.iv
    val ciphertext = cipher.doFinal(plaintext)

    // Combine IV + ciphertext
    val combined = ByteArray(iv.size + ciphertext.size)
    System.arraycopy(iv, 0, combined, 0, iv.size)
    System.arraycopy(ciphertext, 0, combined, iv.size, ciphertext.size)

    return Base64.getEncoder().encodeToString(combined)
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun decryptShardData(encryptedData: String): ByteArray {
    val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
    keyStore.load(null)

    val shardKeyAlias = "com.cowallet.shard.encryption"
    val secretKey = keyStore.getKey(shardKeyAlias, null)
      ?: throw Exception("Shard encryption key not found")

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
