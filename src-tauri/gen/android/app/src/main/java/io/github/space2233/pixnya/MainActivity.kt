package io.github.space2233.pixnya

import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun dispatchKeyEvent(event: KeyEvent): Boolean {
    if (VolumePageTurn.captureEnabled && VolumePageTurn.isVolumeKey(event.keyCode)) {
      if (event.action == KeyEvent.ACTION_DOWN && event.repeatCount == 0) {
        VolumePageTurn.directionFor(event.keyCode)?.let { VolumePageTurn.dispatch(this, it) }
      }
      return true
    }
    return super.dispatchKeyEvent(event)
  }
}
