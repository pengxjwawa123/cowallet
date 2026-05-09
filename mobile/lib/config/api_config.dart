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
    this.isTestnet = false,
    required this.isL2,
  });

  // Mainnets
  static const ethereum = ChainConfig(
    chainId: 1,
    name: 'ethereum',
    displayName: 'Ethereum',
    symbol: 'ETH',
    isL2: false,
  );

  static const base = ChainConfig(
    chainId: 8453,
    name: 'base',
    displayName: 'Base',
    symbol: 'ETH',
    isL2: true,
  );

  static const arbitrum = ChainConfig(
    chainId: 42161,
    name: 'arbitrum',
    displayName: 'Arbitrum One',
    symbol: 'ETH',
    isL2: true,
  );

  static const optimism = ChainConfig(
    chainId: 10,
    name: 'optimism',
    displayName: 'Optimism',
    symbol: 'ETH',
    isL2: true,
  );

  static const bsc = ChainConfig(
    chainId: 56,
    name: 'bsc',
    displayName: 'BNB Chain',
    symbol: 'BNB',
    isL2: false,
  );

  static const polygon = ChainConfig(
    chainId: 137,
    name: 'polygon',
    displayName: 'Polygon',
    symbol: 'POL',
    isL2: false,
  );

  // Testnets
  static const baseSepolia = ChainConfig(
    chainId: 84532,
    name: 'base-sepolia',
    displayName: 'Base Sepolia',
    symbol: 'ETH',
    isTestnet: true,
    isL2: true,
  );

  static const ethereumSepolia = ChainConfig(
    chainId: 11155111,
    name: 'sepolia',
    displayName: 'Ethereum Sepolia',
    symbol: 'ETH',
    isTestnet: true,
    isL2: false,
  );

  /// All mainnet chains
  static const List<ChainConfig> allMainnets = [
    ethereum,
    base,
    arbitrum,
    optimism,
    bsc,
    polygon,
  ];

  /// All testnet chains
  static const List<ChainConfig> allTestnets = [
    baseSepolia,
    ethereumSepolia,
  ];

  /// All supported chains
  static const List<ChainConfig> allChains = [
    ...allMainnets,
    ...allTestnets,
  ];

  /// Dynamically loaded chains from backend (populated on app start)
  static List<ChainConfig> _remoteChains = [];

  /// Whether remote chains have been loaded
  static bool get hasRemoteChains => _remoteChains.isNotEmpty;

  /// Load chains from backend response
  static void loadFromRemote(List<Map<String, dynamic>> chainsJson) {
    _remoteChains = chainsJson.map((json) => ChainConfig(
      chainId: json['chain_id'] as int,
      name: json['name'] as String,
      displayName: json['display_name'] as String,
      symbol: json['symbol'] as String,
      isTestnet: json['is_testnet'] as bool? ?? false,
      isL2: json['is_l2'] as bool? ?? false,
    )).toList();
  }

  /// All supported chains (remote if loaded, fallback to static)
  static List<ChainConfig> get supportedChains =>
      _remoteChains.isNotEmpty ? _remoteChains : allChains;

  /// Supported mainnets
  static List<ChainConfig> get supportedMainnets =>
      supportedChains.where((c) => !c.isTestnet).toList();

  /// Supported testnets
  static List<ChainConfig> get supportedTestnets =>
      supportedChains.where((c) => c.isTestnet).toList();

  /// Get chain config by chain ID
  static ChainConfig? byChainId(int chainId) {
    try {
      return supportedChains.firstWhere((chain) => chain.chainId == chainId);
    } catch (_) {
      return null;
    }
  }

  /// Default chain (Base)
  static const ChainConfig defaultChain = base;
}
