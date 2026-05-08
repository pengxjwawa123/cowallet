import 'dart:io';
import 'package:flutter/services.dart';
import 'package:google_sign_in/google_sign_in.dart';
import 'package:dio/dio.dart';

/// Cloud backup service for storing encrypted shard data.
/// iOS: iCloud Keychain (MethodChannel → native Swift)
/// Android: Google Drive App Data folder (hidden, app-private, cross-device sync)
abstract class CloudBackupService {
  Future<bool> isAvailable();
  Future<void> store(String key, String encryptedData);
  Future<String?> retrieve(String key);
  Future<void> delete(String key);
}

class PlatformCloudBackup implements CloudBackupService {
  late final CloudBackupService _impl;

  PlatformCloudBackup() {
    _impl = Platform.isIOS ? _ICloudBackup() : _GoogleDriveBackup();
  }

  @override
  Future<bool> isAvailable() => _impl.isAvailable();
  @override
  Future<void> store(String key, String encryptedData) => _impl.store(key, encryptedData);
  @override
  Future<String?> retrieve(String key) => _impl.retrieve(key);
  @override
  Future<void> delete(String key) => _impl.delete(key);
}

/// iOS: iCloud Keychain via native MethodChannel
class _ICloudBackup implements CloudBackupService {
  static const _channel = MethodChannel('com.cowallet/cloud_backup');

  @override
  Future<bool> isAvailable() async {
    try {
      final result = await _channel.invokeMethod<bool>('isAvailable');
      return result ?? false;
    } catch (_) {
      return false;
    }
  }

  @override
  Future<void> store(String key, String encryptedData) async {
    await _channel.invokeMethod('store', {'key': key, 'data': encryptedData});
  }

  @override
  Future<String?> retrieve(String key) async {
    try {
      return await _channel.invokeMethod<String>('retrieve', {'key': key});
    } on PlatformException {
      return null;
    }
  }

  @override
  Future<void> delete(String key) async {
    await _channel.invokeMethod('delete', {'key': key});
  }
}

/// Android: Google Drive App Data folder
/// Files stored here are hidden from the user, encrypted by Google, and sync across devices.
class _GoogleDriveBackup implements CloudBackupService {
  static const _driveFileScope = 'https://www.googleapis.com/auth/drive.appdata';
  static const _driveUploadUrl = 'https://www.googleapis.com/upload/drive/v3/files';
  static const _driveApiUrl = 'https://www.googleapis.com/drive/v3/files';

  final _googleSignIn = GoogleSignIn(scopes: [_driveFileScope]);
  final _dio = Dio();

  Future<Map<String, String>?> _getAuthHeaders() async {
    var account = _googleSignIn.currentUser;
    account ??= await _googleSignIn.signInSilently();
    account ??= await _googleSignIn.signIn();
    if (account == null) return null;

    final auth = await account.authentication;
    if (auth.accessToken == null) return null;
    return {'Authorization': 'Bearer ${auth.accessToken}'};
  }

  @override
  Future<bool> isAvailable() async {
    try {
      final headers = await _getAuthHeaders();
      return headers != null;
    } catch (_) {
      return false;
    }
  }

  @override
  Future<void> store(String key, String encryptedData) async {
    final headers = await _getAuthHeaders();
    if (headers == null) throw PlatformException(code: 'AUTH_FAILED', message: 'Google Sign-In failed');

    final existingId = await _findFile(key, headers);

    if (existingId != null) {
      await _dio.patch(
        '$_driveUploadUrl/$existingId?uploadType=media',
        data: encryptedData,
        options: Options(headers: {...headers, 'Content-Type': 'text/plain'}),
      );
    } else {
      await _dio.post(
        '$_driveUploadUrl?uploadType=multipart',
        data: FormData.fromMap({
          'metadata': MultipartFile.fromString(
            '{"name":"$key","parents":["appDataFolder"]}',
            contentType: DioMediaType('application', 'json'),
          ),
          'file': MultipartFile.fromString(
            encryptedData,
            contentType: DioMediaType('text', 'plain'),
          ),
        }),
        options: Options(headers: headers),
      );
    }
  }

  @override
  Future<String?> retrieve(String key) async {
    final headers = await _getAuthHeaders();
    if (headers == null) return null;

    final fileId = await _findFile(key, headers);
    if (fileId == null) return null;

    final resp = await _dio.get<String>(
      '$_driveApiUrl/$fileId?alt=media',
      options: Options(headers: headers, responseType: ResponseType.plain),
    );
    return resp.data;
  }

  @override
  Future<void> delete(String key) async {
    final headers = await _getAuthHeaders();
    if (headers == null) return;

    final fileId = await _findFile(key, headers);
    if (fileId == null) return;

    await _dio.delete(
      '$_driveApiUrl/$fileId',
      options: Options(headers: headers),
    );
  }

  Future<String?> _findFile(String name, Map<String, String> headers) async {
    final resp = await _dio.get<Map<String, dynamic>>(
      _driveApiUrl,
      queryParameters: {
        'spaces': 'appDataFolder',
        'q': "name='$name'",
        'fields': 'files(id)',
      },
      options: Options(headers: headers),
    );
    final files = resp.data?['files'] as List?;
    if (files == null || files.isEmpty) return null;
    return files.first['id'] as String?;
  }
}
