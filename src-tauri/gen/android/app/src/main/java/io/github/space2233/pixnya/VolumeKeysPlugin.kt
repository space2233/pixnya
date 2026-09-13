package io.github.space2233.pixnya

import android.app.Activity
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class VolumeCaptureArgs {
  var enabled: Boolean = false
}

object VolumePageTurn {
  @Volatile
  var captureEnabled: Boolean = false

  fun isVolumeKey(keyCode: Int): Boolean {
    return keyCode == KeyEvent.KEYCODE_VOLUME_DOWN || keyCode == KeyEvent.KEYCODE_VOLUME_UP
  }

  fun directionFor(keyCode: Int): String? {
    return when (keyCode) {
      KeyEvent.KEYCODE_VOLUME_DOWN -> "next"
      KeyEvent.KEYCODE_VOLUME_UP -> "previous"
      else -> null
    }
  }

  fun dispatch(activity: Activity, direction: String) {
    val safeDirection = if (direction == "next") "next" else "previous"
    val script =
      "window.dispatchEvent(new CustomEvent('pixnya-volume-page',{detail:{direction:'$safeDirection'}}))"
    activity.runOnUiThread {
      findWebView(activity.window.decorView)?.evaluateJavascript(script, null)
    }
  }

  private fun findWebView(view: View?): WebView? {
    if (view is WebView) return view
    if (view is ViewGroup) {
      for (index in 0 until view.childCount) {
        val found = findWebView(view.getChildAt(index))
        if (found != null) return found
      }
    }
    return null
  }
}

@TauriPlugin
class VolumeKeysPlugin(activity: Activity) : Plugin(activity) {
  @Command
  fun setCaptureEnabled(invoke: Invoke) {
    VolumePageTurn.captureEnabled = invoke.parseArgs(VolumeCaptureArgs::class.java).enabled
    invoke.resolve()
  }
}
