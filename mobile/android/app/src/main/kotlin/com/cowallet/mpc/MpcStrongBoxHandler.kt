package com.cowallet.mpc

import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.annotation.RequiresApi
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.fragment.app.FragmentActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.util.Base64
import javax.crypto.Cipher

class MpcStrongBoxHandler(private val context: Context) : MethodChannel.MethodCallHandler {
  companion object {
    private const val CHANNEL = "com.cowallet.mpc/strongbox"
    private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
    private const val KEYSTORE_ALGORITHM = "RSA"
    private const val KEYSTORE_ALIAS_PREFIX = "com.cowallet.strongbox."
    private const val KEY_SIZE = 2048

    fun setup(flutterEngine: FlutterEngine, context: Context) {
      val channel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL)
      channel.setMethodCallHandler(MpcStrongBoxHandler(context))
    }
  }

  override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
    when (call.method) {
      "isAvailable" -> isAvailable(result)
      "generateKey" -> {
        val keyId = call.argument<String>("keyId")
        if (keyId != null) {
          generateKey(keyId, result)
        } else {
          result.error("INVALID_ARGS", "keyId is required", null)
        }
      }
      "getPublicKey" -> {
        val keyId = call.argument<String>("keyId")
        if (keyId != null) {
          getPublicKey(keyId, result)
        } else {
          result.error("INVALID_ARGS", "keyId is required", null)
        }
      }
      "signWithBiometric" -> {
        val keyId = call.argument<String>("keyId")
        val hash = call.argument<String>("hash")
        val reason = call.argument<String>("reason")
        if (keyId != null && hash != null && reason != null) {
          signWithBiometric(keyId, hash, reason, result)
        } else {
          result.error("INVALID_ARGS", "keyId, hash, and reason are required", null)
        }
      }
      else -> result.notImplemented()
    }
  }

  private fun isAvailable(result: MethodChannel.Result) {
    try {
      val available = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        BiometricManager.from(context).canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG) == BiometricManager.BIOMETRIC_SUCCESS
      } else {
        false
      }
      result.success(available)
    } catch (e: Exception) {
      result.error("CHECK_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun generateKey(keyId: String, result: MethodChannel.Result) {
    try {
      val alias = KEYSTORE_ALIAS_PREFIX + keyId
      val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
      keyStore.load(null)

      if (keyStore.containsAlias(alias)) {
        keyStore.deleteEntry(alias)
      }

      val keyGenSpec = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY)
          .setKeySize(KEY_SIZE)
          .setSignaturePaddings(KeyProperties.SIGNATURE_PADDING_RSA_PKCS1)
          .setDigests(KeyProperties.DIGEST_SHA256)
          .setIsStrongBoxBacked(true)
          .setUserAuthenticationRequired(true)
          .setUserAuthenticationValidityDurationSeconds(300)
          .build()
      } else {
        KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY)
          .setKeySize(KEY_SIZE)
          .setSignaturePaddings(KeyProperties.SIGNATURE_PADDING_RSA_PKCS1)
          .setDigests(KeyProperties.DIGEST_SHA256)
          .build()
      }

      val keyPairGenerator = KeyPairGenerator.getInstance(KEYSTORE_ALGORITHM, KEYSTORE_PROVIDER)
      keyPairGenerator.initialize(keyGenSpec)
      val keyPair = keyPairGenerator.generateKeyPair()

      val publicKeyBase64 = Base64.getEncoder().encodeToString(keyPair.public.encoded)

      result.success(
        mapOf(
          "publicKey" to publicKeyBase64,
          "keyId" to keyId
        )
      )
    } catch (e: Exception) {
      result.error("GENERATION_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun getPublicKey(keyId: String, result: MethodChannel.Result) {
    try {
      val alias = KEYSTORE_ALIAS_PREFIX + keyId
      val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
      keyStore.load(null)

      val certificate = keyStore.getCertificate(alias)
        ?: throw Exception("Key not found: $keyId")

      val publicKey = certificate.publicKey
      val publicKeyBase64 = Base64.getEncoder().encodeToString(publicKey.encoded)

      result.success(publicKeyBase64)
    } catch (e: Exception) {
      result.error("GET_KEY_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.P)
  private fun signWithBiometric(
    keyId: String,
    hashBase64: String,
    reason: String,
    result: MethodChannel.Result
  ) {
    try {
      val activity = context as? FragmentActivity
        ?: throw Exception("Context must be FragmentActivity for biometric")

      val hash = Base64.getDecoder().decode(hashBase64)

      val biometricPrompt = BiometricPrompt(
        activity,
        { runnable -> runnable.run() },
        object : BiometricPrompt.AuthenticationCallback() {
          override fun onAuthenticationSucceeded(authResult: BiometricPrompt.AuthenticationResult) {
            try {
              performSignature(keyId, hash, result)
            } catch (e: Exception) {
              result.error("SIGNING_FAILED", e.message, null)
            }
          }

          override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
            result.error("AUTH_FAILED", "Authentication error: $errString", null)
          }

          override fun onAuthenticationFailed() {
            result.error("AUTH_FAILED", "Authentication failed", null)
          }
        }
      )

      val promptInfo = BiometricPrompt.PromptInfo.Builder()
        .setTitle("Sign Transaction")
        .setSubtitle(reason)
        .setNegativeButtonText("Cancel")
        .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
        .build()

      biometricPrompt.authenticate(promptInfo)
    } catch (e: Exception) {
      result.error("SIGNING_FAILED", e.message, null)
    }
  }

  @RequiresApi(Build.VERSION_CODES.M)
  private fun performSignature(
    keyId: String,
    hash: ByteArray,
    result: MethodChannel.Result
  ) {
    try {
      val alias = KEYSTORE_ALIAS_PREFIX + keyId
      val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
      keyStore.load(null)

      val privateKey = keyStore.getKey(alias, null)
        ?: throw Exception("Key not found: $keyId")

      val cipher = Cipher.getInstance("RSA/ECB/PKCS1Padding")
      cipher.init(Cipher.ENCRYPT_MODE, privateKey)

      val signature = cipher.doFinal(hash)
      val signatureBase64 = Base64.getEncoder().encodeToString(signature)

      result.success(signatureBase64)
    } catch (e: Exception) {
      result.error("SIGNING_FAILED", e.message, null)
    }
  }
}
