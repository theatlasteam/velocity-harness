package app.velocity.harness

import android.os.Bundle
import android.net.wifi.WifiManager
import android.content.Context
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private var multicastLock: WifiManager.MulticastLock? = null
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    multicastLock = wifi.createMulticastLock("velocity-discovery").apply { setReferenceCounted(false); acquire() }
  }
  override fun onDestroy() {
    multicastLock?.let { if (it.isHeld) it.release() }
    super.onDestroy()
  }
}
