import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    // Register MPC platform channels manually
    let controller = window?.rootViewController as! FlutterViewController
    let flutterEngine = controller.engine
    
    MpcSecureEnclaveHandler.register(with: self)
    MpcSecureStorageHandler.register(with: self)
    CloudBackupHandler.register(with: self)

    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    
    // Also register MPC handlers here
    let binaryMessenger = engineBridge.pluginRegistry
    MpcSecureEnclaveHandler.register(with: binaryMessenger)
    MpcSecureStorageHandler.register(with: binaryMessenger)
    CloudBackupHandler.register(with: binaryMessenger)
  }
}
