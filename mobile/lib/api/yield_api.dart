import '../network/dio_client.dart';
import '../network/result.dart';

/// Yield/DeFi API - matches backend /yield/* routes
class YieldApi {
  /// Search yield opportunities with optional filters.
  /// Returns a YieldSearchResponse with opportunities, total_count, best_apy, average_apy.
  static Future<Result<Map<String, dynamic>>> search({
    int? chainId,
    String? protocolType,
    double? minApy,
    String? maxRisk,
    String? token,
    int? limit,
  }) async {
    final params = <String, dynamic>{};
    if (chainId != null) params['chain_id'] = chainId;
    if (protocolType != null) params['protocol_type'] = protocolType;
    if (minApy != null) params['min_apy'] = minApy;
    if (maxRisk != null) params['max_risk'] = maxRisk;
    if (token != null) params['token'] = token;
    if (limit != null) params['limit'] = limit;

    return await DioClient.get('/yield/search', params: params);
  }

  /// List supported protocols.
  /// Returns a ProtocolsResponse with protocols array.
  static Future<Result<Map<String, dynamic>>> getProtocols({
    int? chainId,
    String? protocolType,
  }) async {
    final params = <String, dynamic>{};
    if (chainId != null) params['chain_id'] = chainId;
    if (protocolType != null) params['protocol_type'] = protocolType;

    return await DioClient.get('/yield/protocols', params: params);
  }
}

/// Parsed yield opportunity from API response.
class YieldOpportunity {
  final String id;
  final String protocolId;
  final String protocolName;
  final int chainId;
  final String opportunityType;
  final TokenInfo? tokenA;
  final TokenInfo? tokenB;
  final double apy;
  final ApyBreakdown apyBreakdown;
  final double tvlUsd;
  final double? volume24hUsd;
  final String riskLevel;
  final List<String> riskFactors;
  final String? strategy;
  final int? lockDays;
  final String smartContractAddress;
  final String updatedAt;

  const YieldOpportunity({
    required this.id,
    required this.protocolId,
    required this.protocolName,
    required this.chainId,
    required this.opportunityType,
    this.tokenA,
    this.tokenB,
    required this.apy,
    required this.apyBreakdown,
    required this.tvlUsd,
    this.volume24hUsd,
    required this.riskLevel,
    required this.riskFactors,
    this.strategy,
    this.lockDays,
    required this.smartContractAddress,
    required this.updatedAt,
  });

  factory YieldOpportunity.fromJson(Map<String, dynamic> json) {
    return YieldOpportunity(
      id: json['id'] ?? '',
      protocolId: json['protocol_id'] ?? '',
      protocolName: json['protocol_name'] ?? '',
      chainId: json['chain_id'] ?? 8453,
      opportunityType: json['opportunity_type'] ?? 'farm',
      tokenA: json['token_a'] != null ? TokenInfo.fromJson(json['token_a']) : null,
      tokenB: json['token_b'] != null ? TokenInfo.fromJson(json['token_b']) : null,
      apy: (json['apy'] ?? 0).toDouble(),
      apyBreakdown: ApyBreakdown.fromJson(json['apy_breakdown'] ?? {}),
      tvlUsd: (json['tvl_usd'] ?? 0).toDouble(),
      volume24hUsd: json['volume_24h_usd']?.toDouble(),
      riskLevel: json['risk_level'] ?? 'medium',
      riskFactors: List<String>.from(json['risk_factors'] ?? []),
      strategy: json['strategy'],
      lockDays: json['lock_days'],
      smartContractAddress: json['smart_contract_address'] ?? '',
      updatedAt: json['updated_at'] ?? '',
    );
  }

  /// Display label for the token pair (e.g. "WETH" or "WETH / USDC")
  String get tokenLabel {
    final a = tokenA?.symbol ?? '';
    final b = tokenB?.symbol;
    if (b != null && b.isNotEmpty) return '$a / $b';
    return a;
  }

  /// Human-readable protocol type
  String get typeLabel {
    switch (opportunityType) {
      case 'dex':
        return 'DEX';
      case 'lending':
        return 'Lending';
      case 'liquid_staking':
        return 'Staking';
      case 'vault':
        return 'Vault';
      case 'farm':
        return 'Farm';
      default:
        return opportunityType;
    }
  }
}

class TokenInfo {
  final String address;
  final String symbol;
  final String name;
  final int decimals;
  final double? priceUsd;

  const TokenInfo({
    required this.address,
    required this.symbol,
    required this.name,
    required this.decimals,
    this.priceUsd,
  });

  factory TokenInfo.fromJson(Map<String, dynamic> json) {
    return TokenInfo(
      address: json['address'] ?? '',
      symbol: json['symbol'] ?? '',
      name: json['name'] ?? '',
      decimals: json['decimals'] ?? 18,
      priceUsd: json['price_usd']?.toDouble(),
    );
  }
}

class ApyBreakdown {
  final double baseApy;
  final double rewardApy;
  final double incentiveApy;
  final double totalApy;

  const ApyBreakdown({
    required this.baseApy,
    required this.rewardApy,
    required this.incentiveApy,
    required this.totalApy,
  });

  factory ApyBreakdown.fromJson(Map<String, dynamic> json) {
    return ApyBreakdown(
      baseApy: (json['base_apy'] ?? 0).toDouble(),
      rewardApy: (json['reward_apy'] ?? 0).toDouble(),
      incentiveApy: (json['incentive_apy'] ?? 0).toDouble(),
      totalApy: (json['total_apy'] ?? 0).toDouble(),
    );
  }
}

class ProtocolInfo {
  final String id;
  final String name;
  final int chainId;
  final String protocolType;
  final double tvlUsd;
  final List<double> apyRange;
  final String riskLevel;
  final int auditCount;
  final int daysActive;

  const ProtocolInfo({
    required this.id,
    required this.name,
    required this.chainId,
    required this.protocolType,
    required this.tvlUsd,
    required this.apyRange,
    required this.riskLevel,
    required this.auditCount,
    required this.daysActive,
  });

  factory ProtocolInfo.fromJson(Map<String, dynamic> json) {
    return ProtocolInfo(
      id: json['id'] ?? '',
      name: json['name'] ?? '',
      chainId: json['chain_id'] ?? 8453,
      protocolType: json['protocol_type'] ?? '',
      tvlUsd: (json['tvl_usd'] ?? 0).toDouble(),
      apyRange: (json['apy_range'] as List<dynamic>?)
              ?.map((e) => (e as num).toDouble())
              .toList() ??
          [0, 0],
      riskLevel: json['risk_level'] ?? 'medium',
      auditCount: json['audit_count'] ?? 0,
      daysActive: json['days_active'] ?? 0,
    );
  }
}
