import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../api/tx_api.dart';
import '../../services/locator.dart';
import '../../main.dart';
import '../../config/api_config.dart';
import '../../router/app_router.dart';

class TxHistoryView extends StatefulWidget {
  const TxHistoryView({super.key});

  @override
  State<TxHistoryView> createState() => _TxHistoryViewState();
}

class _TxHistoryViewState extends State<TxHistoryView> {
  List<dynamic> _transactions = [];
  bool _loading = true;
  bool _loadingMore = false;
  String? _error;
  int _offset = 0;
  final int _limit = 50;
  bool _hasMore = true;
  int? _selectedChainId;
  final ScrollController _scrollController = ScrollController();

  // Chain options for filtering
  final List<Map<String, dynamic>> _chains = [
    {'id': null, 'name': 'All Chains'},
    {'id': 1, 'name': 'Ethereum'},
    {'id': 8453, 'name': 'Base'},
    {'id': 42161, 'name': 'Arbitrum'},
    {'id': 10, 'name': 'Optimism'},
    {'id': 56, 'name': 'BNB Chain'},
    {'id': 137, 'name': 'Polygon'},
  ];

  @override
  void initState() {
    super.initState();
    _loadTransactions();
    _scrollController.addListener(_onScroll);
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (_scrollController.position.pixels >=
        _scrollController.position.maxScrollExtent - 200) {
      if (!_loadingMore && _hasMore) {
        _loadMoreTransactions();
      }
    }
  }

  Future<void> _loadTransactions() async {
    setState(() {
      _loading = true;
      _error = null;
      _offset = 0;
      _hasMore = true;
    });

    try {
      final address = await Services.mpcWallet.getAddress();
      final result = await TxApi.getTransactionHistory(
        address,
        chainId: _selectedChainId,
        limit: _limit,
        offset: 0,
      );

      if (result.isSuccess && result.data != null) {
        final transactions = result.data!['transactions'] as List<dynamic>? ?? [];
        final total = result.data!['total'] as int? ?? 0;

        if (mounted) {
          setState(() {
            _transactions = transactions;
            _loading = false;
            _offset = transactions.length;
            _hasMore = _offset < total;
          });
        }
      } else {
        if (mounted) {
          setState(() {
            _loading = false;
            _error = result.errorMessage ?? 'Failed to load transactions';
          });
        }
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = e.toString();
        });
      }
    }
  }

  Future<void> _loadMoreTransactions() async {
    if (_loadingMore) return;

    setState(() => _loadingMore = true);

    try {
      final address = await Services.mpcWallet.getAddress();
      final result = await TxApi.getTransactionHistory(
        address,
        chainId: _selectedChainId,
        limit: _limit,
        offset: _offset,
      );

      if (result.isSuccess && result.data != null) {
        final transactions = result.data!['transactions'] as List<dynamic>? ?? [];
        final total = result.data!['total'] as int? ?? 0;

        if (mounted) {
          setState(() {
            _transactions.addAll(transactions);
            _loadingMore = false;
            _offset += transactions.length;
            _hasMore = _offset < total;
          });
        }
      } else {
        if (mounted) {
          setState(() => _loadingMore = false);
        }
      }
    } catch (e) {
      if (mounted) {
        setState(() => _loadingMore = false);
      }
    }
  }

  void _showTransactionDetail(Map<String, dynamic> tx) {
    final walletAddress = CowalletApp.of(context).walletAddress;
    final to = tx['to'] as String? ?? '';
    final isIncoming = to.toLowerCase() == walletAddress.toLowerCase();
    final chainId = tx['chain_id'] as int? ?? 1;
    final tokenSymbol = _getTokenSymbol(tx, chainId);

    AppShell.goToChatAndShowTx(context, {
      'tx_hash': tx['tx_hash'] ?? '',
      'from': tx['from'] ?? '',
      'to': to,
      'value': tx['value'] ?? '0',
      'token': tokenSymbol,
      'status': tx['status'] ?? '',
      'chain_id': chainId,
      'block_number': tx['block_number'],
      'timestamp': tx['timestamp'],
      'gas_used': tx['gas_used'],
      'is_incoming': isIncoming,
    });
  }

  String _getTokenSymbol(Map<String, dynamic> tx, int chainId) {
    final symbol = tx['token_symbol'] as String?;
    if (symbol != null && symbol.isNotEmpty) return symbol;
    if (chainId == 137) return 'POL';
    if (chainId == 56) return 'BNB';
    return 'ETH';
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: CwColors.bgPaper,
      appBar: AppBar(
        title: const Text('Transaction History'),
        backgroundColor: CwColors.bgPaper,
        elevation: 0,
      ),
      body: RefreshIndicator(
        onRefresh: _loadTransactions,
        child: Column(
          children: [
            // Chain filter chips
            if (_chains.length > 1) _buildChainFilterChips(),
            Expanded(child: _buildBody()),
          ],
        ),
      ),
    );
  }

  Widget _buildChainFilterChips() {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: Row(
          children: _chains.map((chain) {
            final isSelected = _selectedChainId == chain['id'];
            final chainConfig = chain['id'] != null ? ChainConfig.byChainId(chain['id']) : null;
            final chainColor = chainConfig != null ? _chainColor(chainConfig) : CwColors.accent;

            return Padding(
              padding: const EdgeInsets.only(right: 8),
              child: FilterChip(
                selected: isSelected,
                label: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (chain['id'] != null) ...[
                      Container(
                        width: 8,
                        height: 8,
                        decoration: BoxDecoration(
                          color: chainColor,
                          shape: BoxShape.circle,
                        ),
                      ),
                      const SizedBox(width: 6),
                    ],
                    Text(chain['name']),
                  ],
                ),
                onSelected: (selected) {
                  setState(() {
                    _selectedChainId = chain['id'];
                  });
                  _loadTransactions();
                },
                backgroundColor: CwColors.bgCard,
                selectedColor: chainColor.withValues(alpha: 0.15),
                checkmarkColor: chainColor,
                labelStyle: TextStyle(
                  fontSize: 13,
                  fontWeight: isSelected ? FontWeight.w600 : FontWeight.w500,
                  color: isSelected ? chainColor : CwColors.ink2,
                ),
                side: BorderSide(
                  color: isSelected ? chainColor : CwColors.line,
                  width: isSelected ? 1.5 : 1,
                ),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(20),
                ),
              ),
            );
          }).toList(),
        ),
      ),
    );
  }

  static Color _chainColor(ChainConfig chain) {
    switch (chain.name) {
      case 'ethereum':
      case 'sepolia':
        return const Color(0xFF627EEA);
      case 'base':
      case 'base-sepolia':
        return const Color(0xFF0052FF);
      case 'arbitrum':
        return const Color(0xFF28A0F0);
      case 'optimism':
        return const Color(0xFFFF0420);
      case 'bsc':
        return const Color(0xFFF3BA2F);
      case 'polygon':
        return const Color(0xFF8247E5);
      default:
        return CwColors.ink3;
    }
  }

  Widget _buildBody() {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }

    if (_error != null) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.error_outline, size: 64, color: CwColors.danger),
            const SizedBox(height: 16),
            Text(
              _error!,
              style: const TextStyle(color: CwColors.danger),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 16),
            ElevatedButton(
              onPressed: _loadTransactions,
              child: const Text('Retry'),
            ),
          ],
        ),
      );
    }

    if (_transactions.isEmpty) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.receipt_long, size: 64, color: CwColors.ink3),
            const SizedBox(height: 16),
            Text(
              S.noTxYet,
              style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    color: CwColors.ink2,
                  ),
            ),
          ],
        ),
      );
    }

    return ListView.builder(
      controller: _scrollController,
      padding: const EdgeInsets.all(16),
      itemCount: _transactions.length + (_loadingMore ? 1 : 0),
      itemBuilder: (context, index) {
        if (index == _transactions.length) {
          return const Center(
            child: Padding(
              padding: EdgeInsets.all(16),
              child: CircularProgressIndicator(),
            ),
          );
        }

        final tx = _transactions[index] as Map<String, dynamic>;
        return _TransactionItem(
          tx: tx,
          walletAddress: CowalletApp.of(context).walletAddress,
          onTap: () => _showTransactionDetail(tx),
        );
      },
    );
  }
}

class _TransactionItem extends StatelessWidget {
  final Map<String, dynamic> tx;
  final String walletAddress;
  final VoidCallback onTap;

  const _TransactionItem({
    required this.tx,
    required this.walletAddress,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final from = tx['from'] as String? ?? '';
    final to = tx['to'] as String? ?? '';
    final value = tx['value'] as String? ?? '0';
    final status = tx['status'] as String? ?? '';
    final timestamp = tx['timestamp'] as String?;
    final blockNumber = tx['block_number'] as int?;
    final chainId = tx['chain_id'] as int? ?? 1;

    final isIncoming = to.toLowerCase() == walletAddress.toLowerCase();
    final chain = ChainConfig.byChainId(chainId);
    final chainColor = chain != null ? _TxHistoryViewState._chainColor(chain) : CwColors.ink3;

    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: CwColors.line),
      ),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Row(
            children: [
              // Direction icon
              Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  color: isIncoming
                      ? CwColors.successSoft
                      : CwColors.ink3.withValues(alpha: 0.1),
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  isIncoming ? Icons.arrow_downward : Icons.arrow_upward,
                  color: isIncoming ? CwColors.success : CwColors.ink2,
                  size: 20,
                ),
              ),
              const SizedBox(width: 12),

              // Address and token info
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Container(
                          width: 6,
                          height: 6,
                          decoration: BoxDecoration(
                            color: chainColor,
                            shape: BoxShape.circle,
                          ),
                        ),
                        const SizedBox(width: 6),
                        Text(
                          chain?.displayName ?? (tx['chain_name'] as String? ?? 'Chain $chainId'),
                          style: TextStyle(
                            fontFamily: 'Inter',
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: chainColor,
                          ),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            _truncateAddress(isIncoming ? from : to),
                            style: const TextStyle(
                              fontFamily: 'JetBrainsMono',
                              fontSize: 13,
                              fontWeight: FontWeight.w600,
                              color: CwColors.ink1,
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Text(
                          _getTokenSymbol(tx),
                          style: const TextStyle(
                            fontSize: 12,
                            color: CwColors.ink3,
                          ),
                        ),
                        if (blockNumber != null) ...[
                          const Text(' · ', style: TextStyle(color: CwColors.ink4)),
                          Text(
                            'Block $blockNumber',
                            style: const TextStyle(
                              fontSize: 11,
                              color: CwColors.ink4,
                            ),
                          ),
                        ],
                      ],
                    ),
                    if (timestamp != null) ...[
                      const SizedBox(height: 2),
                      Text(
                        _formatTimestamp(timestamp),
                        style: const TextStyle(
                          fontSize: 11,
                          color: CwColors.ink4,
                        ),
                      ),
                    ],
                  ],
                ),
              ),

              // Value and status
              Column(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Text(
                    _formatValue(value, _getTokenSymbol(tx)),
                    style: TextStyle(
                      fontFamily: 'JetBrainsMono',
                      fontSize: 14,
                      fontWeight: FontWeight.w700,
                      color: isIncoming ? CwColors.success : CwColors.ink1,
                    ),
                  ),
                  const SizedBox(height: 4),
                  _StatusBadge(status: status),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  String _truncateAddress(String addr) {
    if (addr.length <= 10) return addr;
    return '${addr.substring(0, 6)}...${addr.substring(addr.length - 4)}';
  }

  String _getTokenSymbol(Map<String, dynamic> tx) {
    final symbol = tx['token_symbol'] as String?;
    if (symbol != null && symbol.isNotEmpty) return symbol;
    final tokenAddress = tx['token_address'] as String?;
    if (tokenAddress == null || tokenAddress == 'native') {
      final chainId = tx['chain_id'] as int? ?? 1;
      if (chainId == 137) return 'POL';
      if (chainId == 56) return 'BNB';
      return 'ETH';
    }
    return 'Token';
  }

  String _formatValue(String value, String symbol) {
    try {
      final val = BigInt.parse(value);
      final divisor18 = BigInt.from(10).pow(18);
      final whole = val ~/ divisor18;
      final frac = val % divisor18;
      if (whole == BigInt.zero) {
        final divisor15 = BigInt.from(10).pow(15);
        final milli = val ~/ divisor15;
        if (milli == BigInt.zero) return '< 0.001 $symbol';
        return '0.${milli.toString().padLeft(3, '0')} $symbol';
      }
      if (frac == BigInt.zero) return '$whole $symbol';
      final fracStr = (frac * BigInt.from(1000) ~/ divisor18).toString().padLeft(3, '0');
      return '$whole.$fracStr $symbol';
    } catch (e) {
      return value;
    }
  }

  String _formatTimestamp(String timestamp) {
    try {
      final dt = DateTime.parse(timestamp);
      final now = DateTime.now();
      final diff = now.difference(dt);

      if (diff.inMinutes < 60) {
        return '${diff.inMinutes}m ago';
      } else if (diff.inHours < 24) {
        return '${diff.inHours}h ago';
      } else if (diff.inDays < 7) {
        return '${diff.inDays}d ago';
      } else {
        return '${dt.month}/${dt.day}/${dt.year}';
      }
    } catch (e) {
      return timestamp;
    }
  }
}

class _StatusBadge extends StatelessWidget {
  final String status;

  const _StatusBadge({required this.status});

  @override
  Widget build(BuildContext context) {
    Color color;
    String label;

    switch (status.toLowerCase()) {
      case 'confirmed':
        color = CwColors.success;
        label = S.txConfirmed;
        break;
      case 'pending':
      case 'broadcast':
        color = CwColors.warn;
        label = S.txPending;
        break;
      case 'failed':
        color = CwColors.danger;
        label = S.txFailedStatus;
        break;
      default:
        color = CwColors.ink3;
        label = status;
    }

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: color,
        ),
      ),
    );
  }
}

