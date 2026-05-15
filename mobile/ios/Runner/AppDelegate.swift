import Flutter
import UIKit
import FirebaseCore
import FirebaseMessaging

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate, MessagingDelegate {
  override func application(
    
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    // Initialize Firebase
    FirebaseApp.configure()

    // Set Firebase Messaging delegate
    Messaging.messaging().delegate = self

    // Register for remote notifications
    if #available(iOS 10.0, *) {
      UNUserNotificationCenter.current().delegate = self
    }

    // Register MPC platform channels manually
    let controller = window?.rootViewController as! FlutterViewController
    let flutterEngine = controller.engine

    MpcSecureEnclaveHandler.register(with: self)
    MpcSecureStorageHandler.register(with: self)
    CloudBackupHandler.register(with: self)

    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  // MARK: - Firebase Messaging Delegate

  func messaging(_ messaging: Messaging, didReceiveRegistrationToken fcmToken: String?) {
    if let token = fcmToken {
      print("[FCM] Registration token: \(token)")
      // Token will be sent to backend from Flutter side
    }
  }

  // Handle remote notifications
  override func application(
    _ application: UIApplication,
    didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
  ) {
    Messaging.messaging().apnsToken = deviceToken
  }

  override func application(
    _ application: UIApplication,
    didFailToRegisterForRemoteNotificationsWithError error: Error
  ) {
    print("[FCM] Failed to register for remote notifications: \(error)")
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
