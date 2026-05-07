class ApiConfig {
  // 后端API地址
  static const String baseUrl = "http://43.163.101.37:3000";

  // API统一前缀
  static const String apiPrefix = "/api/v1";

  // 完整API基础地址
  static const String apiBaseUrl = "$baseUrl$apiPrefix";

  // WebSocket基础地址（http→ws, https→wss）
  static String get wsBaseUrl =>
      baseUrl.replaceFirst('http://', 'ws://').replaceFirst('https://', 'wss://');

  // 完整WebSocket基础地址
  static String get wsApiBaseUrl => "$wsBaseUrl$apiPrefix";

  // 连接超时时间（秒）
  static const int connectTimeout = 15;

  // 响应超时时间（秒）
  static const int receiveTimeout = 15;

  // 健康检查接口
  static const String healthCheck = "$baseUrl/health";
}

/// Supported blockchain networks configuration
class ChainConfig {
  final int chainId;
  final String name;
  final String displayName;
  final String symbol;
  final bool isTestnet;
  final bool isL2;

  const ChainConfig({
    required this.chainId,
    required this.name,
    required this.displayName,
    required this.symbol,
    required this.isTestnet,
    required this.isL2,
  });

  // Mainnet chains
  static const ethereumMainnet = ChainConfig(
    chainId: 1,
    name: 'ethereum',
    displayName: 'Ethereum',
    symbol: 'ETH',
    isTestnet: false,
    isL2: false,
  );

  static const baseMainnet = ChainConfig(
    chainId: 8453,
    name: 'base',
    displayName: 'Base',
    symbol: 'ETH',
    isTestnet: false,
    isL2: true,
  );

  static const arbitrumOne = ChainConfig(
    chainId: 42161,
    name: 'arbitrum',
    displayName: 'Arbitrum One',
    symbol: 'ETH',
    isTestnet: false,
    isL2: true,
  );

  static const optimismMainnet = ChainConfig(
    chainId: 10,
    name: 'optimism',
    displayName: 'Optimism',
    symbol: 'ETH',
    isTestnet: false,
    isL2: true,
  );

  static const bnbChain = ChainConfig(
    chainId: 56,
    name: 'bsc',
    displayName: 'BNB Chain',
    symbol: 'BNB',
    isTestnet: false,
    isL2: false,
  );

  // Testnet chains
  static const ethereumSepolia = ChainConfig(
    chainId: 11155111,
    name: 'sepolia',
    displayName: 'Ethereum Sepolia',
    symbol: 'ETH',
    isTestnet: true,
    isL2: false,
  );

  static const baseSepolia = ChainConfig(
    chainId: 84532,
    name: 'base-sepolia',
    displayName: 'Base Sepolia',
    symbol: 'ETH',
    isTestnet: true,
    isL2: true,
  );

  /// All supported mainnet chains
  static const List<ChainConfig> allMainnets = [
    ethereumMainnet,
    baseMainnet,
    arbitrumOne,
    optimismMainnet,
    bnbChain,
  ];

  /// All supported testnet chains
  static const List<ChainConfig> allTestnets = [
    ethereumSepolia,
    baseSepolia,
  ];

  /// All supported chains (mainnet + testnet)
  static const List<ChainConfig> allChains = [
    ...allMainnets,
    ...allTestnets,
  ];

  /// Get chain config by chain ID
  static ChainConfig? byChainId(int chainId) {
    try {
      return allChains.firstWhere((chain) => chain.chainId == chainId);
    } catch (_) {
      return null;
    }
  }

  /// Default chain for new wallets (Base Sepolia testnet)
  static const ChainConfig defaultChain = baseSepolia;
}
