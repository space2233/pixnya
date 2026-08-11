package io.github.space2233.pixnya

import android.app.Activity
import android.content.Intent
import android.net.Uri
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class ExternalLinkArgs {
  var url: String? = null
}

@TauriPlugin
class ExternalLinkPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun openUrl(invoke: Invoke) {
    val value = invoke.parseArgs(ExternalLinkArgs::class.java).url
      ?: return invoke.reject("Missing URL", "invalid_url")
    val uri = Uri.parse(value)
    if (uri.scheme != "https" || uri.host != "www.pixiv.net" || uri.userInfo != null) {
      invoke.reject("Only https://www.pixiv.net links are allowed", "invalid_url")
      return
    }
    activity.runOnUiThread {
      try {
        activity.startActivity(Intent(Intent.ACTION_VIEW, uri))
        invoke.resolve()
      } catch (error: Exception) {
        invoke.reject("Unable to open the Pixiv URL", "open_url_failed", error)
      }
    }
  }
}
