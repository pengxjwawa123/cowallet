package com.cowallet.cowallet

import android.os.Build
import androidx.annotation.RequiresApi
import io.flutter.embedding.android.FlutterFragmentActivity
import io.flutter.embedding.engine.FlutterEngine
import com.cowallet.mpc.MpcStrongBoxHandler
import com.cowallet.mpc.MpcKeystoreHandler

class MainActivity : FlutterFragmentActivity() {
  @RequiresApi(Build.VERSION_CODES.M)
  override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
    super.configureFlutterEngine(flutterEngine)

    MpcStrongBoxHandler.setup(flutterEngine, this)
    MpcKeystoreHandler.setup(flutterEngine, this)
  }
}
