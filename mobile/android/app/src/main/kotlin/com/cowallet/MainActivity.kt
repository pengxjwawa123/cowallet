package com.cowallet

import android.os.Build
import androidx.annotation.RequiresApi
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import com.cowallet.mpc.MpcStrongBoxHandler
import com.cowallet.mpc.MpcKeystoreHandler
import com.cowallet.mpc.CloudBackupHandler

class MainActivity : FlutterActivity() {
  @RequiresApi(Build.VERSION_CODES.M)
  override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
    super.configureFlutterEngine(flutterEngine)

    // Register StrongBox and Keystore channel handlers
    MpcStrongBoxHandler.setup(flutterEngine, this)
    MpcKeystoreHandler.setup(flutterEngine, this)
    CloudBackupHandler.setup(flutterEngine, this)
  }
}
