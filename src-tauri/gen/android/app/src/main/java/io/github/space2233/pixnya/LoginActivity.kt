package io.github.space2233.pixnya

import android.annotation.SuppressLint
import android.graphics.Bitmap
import android.net.Uri
import android.net.http.SslCertificate
import android.net.http.SslError
import android.os.Build
import android.os.Bundle
import android.view.View
import android.webkit.CookieManager
import android.webkit.SslErrorHandler
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.ImageButton
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.updatePadding
import androidx.webkit.ProxyConfig
import androidx.webkit.ProxyController
import androidx.webkit.WebViewFeature
import java.security.MessageDigest
import java.lang.ref.WeakReference
import java.util.concurrent.Executor

class LoginActivity : AppCompatActivity() {
  private lateinit var webView: WebView
  private lateinit var progress: ProgressBar
  private lateinit var status: TextView
  private var mode: String = MODE_STANDARD
  private var launchId: Long = 0
  private var bridgeCertSha256: String? = null
  private var proxyApplied = false
  @Volatile private var closing = false
  private val directExecutor = Executor { command -> command.run() }

  @SuppressLint("SetJavaScriptEnabled")
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    activeActivity = WeakReference(this)
    setContentView(R.layout.activity_login)

    val loginRoot = findViewById<View>(R.id.login_root)
    ViewCompat.setOnApplyWindowInsetsListener(loginRoot) { view, insets ->
      val systemBars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
      )
      view.updatePadding(
        left = systemBars.left,
        top = systemBars.top,
        right = systemBars.right,
        bottom = systemBars.bottom,
      )
      insets
    }
    ViewCompat.requestApplyInsets(loginRoot)

    webView = findViewById(R.id.login_webview)
    progress = findViewById(R.id.login_progress)
    status = findViewById(R.id.login_status)
    launchId = intent.getLongExtra(EXTRA_LAUNCH_ID, 0)
    mode = intent.getStringExtra(EXTRA_MODE) ?: MODE_STANDARD
    bridgeCertSha256 = intent.getStringExtra(EXTRA_BRIDGE_CERT_SHA256)
    val url = intent.getStringExtra(EXTRA_URL)

    findViewById<ImageButton>(R.id.login_close).setOnClickListener { finish() }
    findViewById<TextView>(R.id.login_mode).text = modeLabel(mode)
    onBackPressedDispatcher.addCallback(
      this,
      object : OnBackPressedCallback(true) {
        override fun handleOnBackPressed() {
          if (webView.canGoBack()) webView.goBack() else finish()
        }
      },
    )

    if (!isExpectedLoginUrl(url)) {
      showError("登录地址校验失败，页面未加载。")
      return
    }

    webView.settings.apply {
      javaScriptEnabled = true
      domStorageEnabled = true
      allowFileAccess = false
      allowContentAccess = false
      mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
      setSupportMultipleWindows(false)
      javaScriptCanOpenWindowsAutomatically = false
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) safeBrowsingEnabled = true
    }
    webView.removeJavascriptInterface("searchBoxJavaBridge_")
    webView.removeJavascriptInterface("accessibility")
    webView.removeJavascriptInterface("accessibilityTraversal")
    CookieManager.getInstance().apply {
      setAcceptCookie(true)
      setAcceptThirdPartyCookies(webView, true)
    }
    webView.webChromeClient = object : WebChromeClient() {
      override fun onProgressChanged(view: WebView, newProgress: Int) {
        progress.progress = newProgress
        progress.visibility = if (newProgress >= 100) View.GONE else View.VISIBLE
      }
    }
    webView.webViewClient = object : WebViewClient() {
      override fun shouldOverrideUrlLoading(
        view: WebView,
        request: WebResourceRequest,
      ): Boolean = handleNavigation(request.url)

      override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
        status.visibility = View.GONE
      }

      override fun onReceivedSslError(
        view: WebView,
        handler: SslErrorHandler,
        error: SslError,
      ) {
        if (
          isBridgeMode() &&
          isAllowedBridgeUrl(error.url) &&
          isPinnedBridgeCertificate(error.certificate)
        ) {
          handler.proceed()
        } else {
          handler.cancel()
          showError("服务器证书验证失败，登录已停止。")
        }
      }

      override fun onReceivedError(
        view: WebView,
        request: WebResourceRequest,
        error: WebResourceError,
      ) {
        if (request.isForMainFrame) {
          showError("官方登录页加载失败，请返回后切换连接方式重试。")
        }
      }
    }

    applyNetworkModeThenLoad(url!!)
  }

  private fun applyNetworkModeThenLoad(url: String) {
    val proxySupported = WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE)
    if (!proxySupported) {
      if (isBridgeMode()) {
        showError("当前 Android System WebView 不支持代理覆盖，无法启用登录桥。")
      } else {
        loadOfficialPage(url)
      }
      return
    }

    val controller = ProxyController.getInstance()
    if (mode == MODE_ECH || mode == MODE_COMPATIBLE) {
      val port = intent.getIntExtra(EXTRA_PROXY_PORT, 0)
      if (port !in 1..65535) {
        showError("低安全登录桥没有就绪，页面未加载。")
        return
      }
      val config = ProxyConfig.Builder()
        .addProxyRule("127.0.0.1:$port")
        .build()
      controller.setProxyOverride(config, directExecutor) {
        if (closing) {
          controller.clearProxyOverride(directExecutor) {}
          return@setProxyOverride
        }
        proxyApplied = true
        runOnUiThread { loadOfficialPage(url) }
      }
    } else {
      controller.clearProxyOverride(directExecutor) {
        runOnUiThread { loadOfficialPage(url) }
      }
    }
  }

  private fun loadOfficialPage(url: String) {
    if (isFinishing || isDestroyed) return
    status.visibility = View.GONE
    webView.visibility = View.VISIBLE
    webView.loadUrl(url)
  }

  private fun handleNavigation(uri: Uri): Boolean {
    if (
      uri.scheme?.lowercase() == "pixiv" &&
      uri.host?.lowercase() == "account" &&
      uri.path == "/login" &&
      launchId > 0
    ) {
      webView.stopLoading()
      LoginResultRegistry.publish(launchId, uri.toString())
      finish()
      return true
    }

    return when (uri.scheme?.lowercase()) {
      "https", "about" -> false
      else -> true
    }
  }

  private fun showError(message: String) {
    runOnUiThread {
      progress.visibility = View.GONE
      status.text = message
      status.visibility = View.VISIBLE
    }
  }

  private fun modeLabel(mode: String): String = when (mode) {
    MODE_ECH -> "ECH 预检 · 低安全登录桥"
    MODE_COMPATIBLE -> "兼容 · 固定 IP / 低安全 TLS"
    else -> "标准 · 系统网络"
  }

  private fun isBridgeMode(): Boolean = mode == MODE_ECH || mode == MODE_COMPATIBLE

  private fun isAllowedBridgeUrl(url: String?): Boolean {
    if (url == null) return false
    val uri = Uri.parse(url)
    return uri.scheme == "https" && uri.host?.lowercase() in BRIDGE_HOSTS
  }

  private fun isPinnedBridgeCertificate(certificate: SslCertificate): Boolean {
    val expected = bridgeCertSha256 ?: return false
    val certificateBytes = SslCertificate.saveState(certificate)
      .getByteArray("x509-certificate")
      ?: return false
    val actual = MessageDigest.getInstance("SHA-256")
      .digest(certificateBytes)
      .joinToString(separator = "") { byte ->
        (byte.toInt() and 0xff).toString(16).padStart(2, '0')
      }
    return actual.equals(expected, ignoreCase = true)
  }

  private fun isExpectedLoginUrl(url: String?): Boolean {
    if (url == null) return false
    val uri = Uri.parse(url)
    return uri.scheme == "https" &&
      uri.host == "app-api.pixiv.net" &&
      uri.path == "/web/v1/login"
  }

  override fun onPause() {
    webView.onPause()
    super.onPause()
  }

  override fun onResume() {
    super.onResume()
    webView.onResume()
  }

  override fun onDestroy() {
    closing = true
    if (proxyApplied && WebViewFeature.isFeatureSupported(WebViewFeature.PROXY_OVERRIDE)) {
      ProxyController.getInstance().clearProxyOverride(directExecutor) {}
    }
    webView.stopLoading()
    webView.clearHistory()
    webView.removeAllViews()
    webView.destroy()
    if (activeActivity?.get() === this) activeActivity = null
    super.onDestroy()
  }

  companion object {
    @Volatile private var activeActivity: WeakReference<LoginActivity>? = null

    fun finishActive() {
      activeActivity?.get()?.runOnUiThread { activeActivity?.get()?.finish() }
    }

    const val EXTRA_LAUNCH_ID = "pixiv.login.launch_id"
    const val EXTRA_URL = "pixiv.login.url"
    const val EXTRA_MODE = "pixiv.login.mode"
    const val EXTRA_PROXY_PORT = "pixiv.login.proxy_port"
    const val EXTRA_BRIDGE_CERT_SHA256 = "pixiv.login.bridge_cert_sha256"
    const val EXTRA_ECH_PREFLIGHT = "pixiv.login.ech_preflight"

    private const val MODE_STANDARD = "standard"
    private const val MODE_ECH = "ech"
    private const val MODE_COMPATIBLE = "compatible"
    private val BRIDGE_HOSTS = setOf(
      "app-api.pixiv.net",
      "oauth.secure.pixiv.net",
      "accounts.pixiv.net",
      "www.pixiv.net",
      "i.pximg.net",
      "s.pximg.net",
    )
  }
}
