import '../network/dio_client.dart';
import '../network/result.dart';

class ChainsApi {
  static Future<Result<List<Map<String, dynamic>>>> getSupportedChains() async {
    final result = await DioClient.get<Map<String, dynamic>>('/chains');
    if (result.isSuccess && result.data != null) {
      final chains = (result.data!['chains'] as List?)
          ?.map((e) => e as Map<String, dynamic>)
          .toList();
      return Result.success(chains ?? []);
    }
    return Result.error(result.errorMessage ?? 'Failed to fetch chains', 0);
  }
}
