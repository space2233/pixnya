package io.github.space2233.pixnya

import android.app.Activity
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.provider.MediaStore
import android.util.Base64
import android.webkit.CookieManager
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewDatabase
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.nio.charset.StandardCharsets
import java.security.KeyStore
import java.util.concurrent.ConcurrentHashMap
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class OpenLoginArgs {
  var launchId: Long = 0
  var url: String? = null
  var mode: String? = null
  var proxyPort: Int? = null
  var bridgeCertSha256: String? = null
  var echPreflight: String? = null
}

@InvokeArg
class LoginResultArgs {
  var launchId: Long = 0
}

@InvokeArg
class SaveRefreshTokenArgs {
  var refreshToken: String? = null
  var connectionMode: String? = null
}

@InvokeArg
class ExportDiagnosticLogArgs {
  var fileName: String? = null
  var contents: String? = null
}

object LoginResultRegistry {
  private val callbacks = ConcurrentHashMap<Long, String>()

  fun publish(launchId: Long, callbackUrl: String) {
    if (launchId > 0) {
      callbacks.clear()
      callbacks[launchId] = callbackUrl
    }
  }

  fun take(launchId: Long): String? = callbacks.remove(launchId)

  fun clear() = callbacks.clear()
}

private object SecureRefreshTokenStore {
  private const val KEYSTORE = "AndroidKeyStore"
  private const val KEY_ALIAS = "io.github.space2233.pixnya.refresh-token.v1"
  private const val PREFERENCES = "pixiv-client-secure-session"
  private const val IV_KEY = "refresh-token-iv"
  private const val CIPHERTEXT_KEY = "refresh-token-ciphertext"
  private const val CONNECTION_MODE_KEY = "connection-mode"
  private const val TRANSFORMATION = "AES/GCM/NoPadding"

  fun save(context: Context, refreshToken: String, connectionMode: String) {
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
    val ciphertext = cipher.doFinal(refreshToken.toByteArray(StandardCharsets.UTF_8))
    val saved = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
      .edit()
      .putString(IV_KEY, Base64.encodeToString(cipher.iv, Base64.NO_WRAP))
      .putString(CIPHERTEXT_KEY, Base64.encodeToString(ciphertext, Base64.NO_WRAP))
      .putString(CONNECTION_MODE_KEY, connectionMode)
      .commit()
    check(saved) { "Secure session storage did not commit" }
  }

  fun load(context: Context): String? {
    val preferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
    val encodedIv = preferences.getString(IV_KEY, null) ?: return null
    val encodedCiphertext = preferences.getString(CIPHERTEXT_KEY, null) ?: return null
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(
      Cipher.DECRYPT_MODE,
      getOrCreateKey(),
      GCMParameterSpec(128, Base64.decode(encodedIv, Base64.NO_WRAP)),
    )
    val plaintext = cipher.doFinal(Base64.decode(encodedCiphertext, Base64.NO_WRAP))
    return String(plaintext, StandardCharsets.UTF_8)
  }

  fun loadConnectionMode(context: Context): String =
    context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
      .getString(CONNECTION_MODE_KEY, "standard") ?: "standard"

  fun delete(context: Context) {
    val deleted = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
      .edit()
      .remove(IV_KEY)
      .remove(CIPHERTEXT_KEY)
      .remove(CONNECTION_MODE_KEY)
      .commit()
    check(deleted) { "Secure session storage did not commit" }
    val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
    if (keyStore.containsAlias(KEY_ALIAS)) keyStore.deleteEntry(KEY_ALIAS)
  }

  private fun getOrCreateKey(): SecretKey {
    val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
    (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }

    val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
    generator.init(
      KeyGenParameterSpec.Builder(
        KEY_ALIAS,
        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
      )
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .build(),
    )
    return generator.generateKey()
  }
}

@TauriPlugin
class LoginWebViewPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun openLogin(invoke: Invoke) {
    val args = invoke.parseArgs(OpenLoginArgs::class.java)
    val url = args.url ?: return invoke.reject("Missing login URL", "invalid_login_url")
    val uri = Uri.parse(url)
    if (
      uri.scheme != "https" ||
      uri.host != "app-api.pixiv.net" ||
      uri.path != "/web/v1/login"
    ) {
      return invoke.reject("Unexpected login URL", "invalid_login_url")
    }

    val mode = args.mode
      ?: return invoke.reject("Missing connection mode", "invalid_connection_mode")
    if (mode !in setOf("standard", "ech", "compatible")) {
      return invoke.reject("Unknown connection mode", "invalid_connection_mode")
    }
    val proxyPort = args.proxyPort
    val usesBridge = mode == "compatible"
    if (usesBridge && (proxyPort == null || proxyPort !in 1..65535)) {
      return invoke.reject("Missing login bridge proxy", "proxy_unavailable")
    }
    val bridgeCertSha256 = args.bridgeCertSha256
    if (usesBridge && !bridgeCertSha256.orEmpty().matches(Regex("[0-9A-Fa-f]{64}"))) {
      return invoke.reject("Missing login bridge certificate pin", "proxy_unavailable")
    }

    activity.runOnUiThread {
      try {
        LoginResultRegistry.clear()
        val intent = Intent(activity, LoginActivity::class.java).apply {
          putExtra(LoginActivity.EXTRA_LAUNCH_ID, args.launchId)
          putExtra(LoginActivity.EXTRA_URL, url)
          putExtra(LoginActivity.EXTRA_MODE, mode)
          putExtra(LoginActivity.EXTRA_PROXY_PORT, proxyPort ?: 0)
          putExtra(LoginActivity.EXTRA_BRIDGE_CERT_SHA256, bridgeCertSha256)
          putExtra(LoginActivity.EXTRA_ECH_PREFLIGHT, args.echPreflight ?: "not_applicable")
        }
        activity.startActivity(intent)
        invoke.resolve()
      } catch (error: Exception) {
        invoke.reject("Unable to open login WebView", "login_activity_unavailable", error)
      }
    }
  }

  @Command
  fun takeLoginResult(invoke: Invoke) {
    val args = invoke.parseArgs(LoginResultArgs::class.java)
    if (args.launchId <= 0) {
      invoke.reject("Invalid login launch", "invalid_login_launch")
      return
    }
    val result = JSObject()
    LoginResultRegistry.take(args.launchId)?.let { result.put("callbackUrl", it) }
    invoke.resolve(result)
  }

  @Command
  fun saveRefreshToken(invoke: Invoke) {
    val args = invoke.parseArgs(SaveRefreshTokenArgs::class.java)
    val token = args.refreshToken
    if (token.isNullOrEmpty()) {
      invoke.reject("Missing refresh token", "invalid_refresh_token")
      return
    }
    val connectionMode = args.connectionMode
    if (connectionMode !in setOf("standard", "ech", "compatible")) {
      invoke.reject("Invalid connection mode", "invalid_connection_mode")
      return
    }
    try {
      SecureRefreshTokenStore.save(activity.applicationContext, token, connectionMode!!)
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject("Secure token storage is unavailable", "secure_storage_unavailable", error)
    }
  }

  @Command
  fun loadRefreshToken(invoke: Invoke) {
    try {
      val result = JSObject()
      SecureRefreshTokenStore.load(activity.applicationContext)
        ?.let {
          result.put("refreshToken", it)
          result.put(
            "connectionMode",
            SecureRefreshTokenStore.loadConnectionMode(activity.applicationContext),
          )
        }
      invoke.resolve(result)
    } catch (error: Exception) {
      invoke.reject("Secure token storage is unavailable", "secure_storage_unavailable", error)
    }
  }

  @Command
  fun deleteRefreshToken(invoke: Invoke) {
    try {
      SecureRefreshTokenStore.delete(activity.applicationContext)
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject("Secure token storage is unavailable", "secure_storage_unavailable", error)
    }
  }

  @Command
  fun clearLocalWebData(invoke: Invoke) {
    activity.runOnUiThread {
      try {
        LoginResultRegistry.clear()
        LoginActivity.finishActive()
        WebStorage.getInstance().deleteAllData()
        WebViewDatabase.getInstance(activity.applicationContext).apply {
          clearFormData()
          clearHttpAuthUsernamePassword()
        }
        WebView(activity).apply {
          clearCache(true)
          clearHistory()
          clearFormData()
          destroy()
        }
        CookieManager.getInstance().removeAllCookies {
          CookieManager.getInstance().flush()
          invoke.resolve()
        }
      } catch (error: Exception) {
        invoke.reject("Unable to clear login WebView data", "webview_data_unavailable", error)
      }
    }
  }

  @Command
  fun exportDiagnosticLog(invoke: Invoke) {
    val args = invoke.parseArgs(ExportDiagnosticLogArgs::class.java)
    val fileName = args.fileName
    val contents = args.contents
    if (
      fileName == null ||
      !fileName.matches(Regex("pixnya-diagnostics-[0-9]+\\.txt")) ||
      contents == null
    ) {
      invoke.reject("Invalid diagnostic export", "invalid_export")
      return
    }
    val bytes = contents.toByteArray(StandardCharsets.UTF_8)
    if (bytes.size > 512 * 1024) {
      invoke.reject("Diagnostic export is too large", "invalid_export")
      return
    }

    try {
      val resolver = activity.contentResolver
      val values = ContentValues().apply {
        put(MediaStore.Downloads.DISPLAY_NAME, fileName)
        put(MediaStore.Downloads.MIME_TYPE, "text/plain")
        put(
          MediaStore.Downloads.RELATIVE_PATH,
        Environment.DIRECTORY_DOWNLOADS + "/PixNya",
        )
        put(MediaStore.Downloads.IS_PENDING, 1)
      }
      val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
        ?: throw IllegalStateException("Unable to create diagnostic export")
      try {
        resolver.openOutputStream(uri, "w")?.use { output -> output.write(bytes) }
          ?: throw IllegalStateException("Unable to open diagnostic export")
        values.clear()
        values.put(MediaStore.Downloads.IS_PENDING, 0)
        resolver.update(uri, values, null, null)
      } catch (error: Exception) {
        resolver.delete(uri, null, null)
        throw error
      }

      val result = JSObject()
      result.put("destination", "Downloads/PixNya/$fileName")
      invoke.resolve(result)
    } catch (error: Exception) {
      invoke.reject("Unable to export diagnostic log", "export_unavailable", error)
    }
  }
}
