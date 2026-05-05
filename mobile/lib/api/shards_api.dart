import '../network/dio_client.dart';
import '../network/result.dart';

/// 密钥分片 API - 后端加密存储
class ShardsApi {
  /// 上传密钥分片到后端加密存储
  /// [location] 'server', 'device', 'backup'
  /// [partyIndex] 0=设备, 1=服务端, 2=备份
  /// [shardHex] 33字节的Shamir分片 (hex编码)
  static Future<Result<Map<String, dynamic>>> uploadShard({
    required String location,
    required int partyIndex,
    required String shardHex,
  }) async {
    return await DioClient.post(
      "/shards/shard",
      data: {
        "location": location,
        "party_index": partyIndex,
        "shard_hex": shardHex,
      },
    );
  }

  /// 从后端获取密钥分片 (自动解密)
  static Future<Result<Map<String, dynamic>>> getShard(String location) async {
    return await DioClient.get("/shards/shard/$location");
  }

  /// 批量上传所有分片 (钱包创建时调用)
  /// 分片 0 → 本地存储不上传
  /// 分片 1 → 上传到 server 位置
  /// 分片 2 → 显示给用户备份后删除
  static Future<void> uploadWalletShards(List<String> shards) async {
    // 只上传分片 1 到后端加密存储
    if (shards.length > 1) {
      final result = await uploadShard(
        location: "server",
        partyIndex: 1,
        shardHex: shards[1],
      );

      if (result.isSuccess) {
        print("✅ Shard 1 uploaded to server successfully");
      } else {
        print("❌ Failed to upload shard 1: ${result.errorMessage}");
        throw Exception("Failed to store server shard: ${result.errorMessage}");
      }
    }
  }
}
