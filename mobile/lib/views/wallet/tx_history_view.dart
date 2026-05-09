import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../theme/colors.dart';
import '../../widgets/top_toast.dart';
import '../../api/tx_api.dart';
import '../../services/locator.dart';

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
    {'id': 84532, 'name': 'Base Sepolia'},
    {'id': 42161, 'name': 'Arbitrum'},
    {'id': 10, 'name': 'Optimism'},
    {'id': 56, 'name': 'BSC'},
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
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      builder: (context) => _TransactionDetailSheet(tx: tx),
    );
  }

  void _changeChainFilter() {
    showModalBottomSheet(
      context: context,
      builder: (context) => Container(
        padding: const EdgeInsets.symmetric(vertical: 20),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Padding(
              padding: EdgeInsets.symmetric(horizontal: 20, vertical: 10),
              child: Text(
                'Filter by Chain',
                style: TextStyle(
                  fontSize: 18,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            const Divider(),
            ..._chains.map((chain) => ListTile(
                  leading: _selectedChainId == chain['id']
                      ? const Icon(Icons.check_circle, color: CwColors.accent)
                      : const Icon(Icons.circle_outlined, color: CwColors.ink3),
                  title: Text(chain['name']),
                  onTap: () {
                    setState(() {
                      _selectedChainId = chain['id'];
                    });
                    Navigator.pop(context);
                    _loadTransactions();
                  },
                )),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: CwColors.bgPaper,
      appBar: AppBar(
        title: const Text('Transaction History'),
        backgroundColor: CwColors.bgPaper,
        elevation: 0,
        actions: [
          IconButton(
            icon: const Icon(Icons.filter_list),
            onPressed: _changeChainFilter,
            tooltip: 'Filter by chain',
          ),
        ],
      ),
      body: RefreshIndicator(
        onRefresh: _loadTransactions,
        child: _buildBody(),
      ),
    );
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
              'No transactions yet',
              style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    color: CwColors.ink2,
                  ),
            ),
            const SizedBox(height: 8),
            Text(
              'Your transaction history will appear here',
              style: Theme.of(context).textTheme.bodySmall,
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
          onTap: () => _showTransactionDetail(tx),
        );
      },
    );
  }
}

class _TransactionItem extends StatelessWidget {
  final Map<String, dynamic> tx;
  final VoidCallback onTap;

  const _TransactionItem({
    required this.tx,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final from = tx['from'] as String? ?? '';
    final to = tx['to'] as String? ?? '';
    final value = tx['value'] as String? ?? '0';
    final tokenAddress = tx['token_address'] as String?;
    final status = tx['status'] as String? ?? '';
    final timestamp = tx['timestamp'] as String?;
    final blockNumber = tx['block_number'] as int?;

    // Determine if incoming or outgoing (simplified - needs actual wallet address)
    final isIncoming = true; // TODO: Compare with wallet address

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
                    Text(
                      _truncateAddress(isIncoming ? from : to),
                      style: const TextStyle(
                        fontFamily: 'JetBrainsMono',
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                        color: CwColors.ink1,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Text(
                          _getTokenSymbol(tokenAddress),
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
                    _formatValue(value),
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

  String _getTokenSymbol(String? tokenAddress) {
    if (tokenAddress == null || tokenAddress == 'native') {
      return 'ETH';
    }
    // TODO: Map token addresses to symbols
    return 'Token';
  }

  String _formatValue(String value) {
    try {
      final val = BigInt.parse(value);
      final divisor18 = BigInt.from(10).pow(18);
      final eth = val ~/ divisor18;
      if (eth < BigInt.one) {
        final divisor15 = BigInt.from(10).pow(15);
        return '${val ~/ divisor15} mETH';
      }
      return '$eth ETH';
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
        label = 'Confirmed';
        break;
      case 'pending':
      case 'broadcast':
        color = CwColors.warn;
        label = 'Pending';
        break;
      case 'failed':
        color = CwColors.danger;
        label = 'Failed';
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

class _TransactionDetailSheet extends StatelessWidget {
  final Map<String, dynamic> tx;

  const _TransactionDetailSheet({required this.tx});

  @override
  Widget build(BuildContext context) {
    final txHash = tx['tx_hash'] as String? ?? '';
    final from = tx['from'] as String? ?? '';
    final to = tx['to'] as String? ?? '';
    final value = tx['value'] as String? ?? '0';
    final chainId = tx['chain_id'] as int? ?? 1;
    final blockNumber = tx['block_number'] as int?;

    return Container(
      decoration: const BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      child: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Handle bar
              Center(
                child: Container(
                  width: 40,
                  height: 4,
                  decoration: BoxDecoration(
                    color: CwColors.ink4,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),
              const SizedBox(height: 20),

              // Title
              const Text(
                'Transaction Details',
                style: TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.w700,
                  color: CwColors.ink1,
                ),
              ),
              const SizedBox(height: 20),

              // Details
              _DetailRow(label: 'From', value: from, copyable: true),
              _DetailRow(label: 'To', value: to, copyable: true),
              _DetailRow(label: 'Value', value: value),
              _DetailRow(label: 'Tx Hash', value: txHash, copyable: true),
              if (blockNumber != null)
                _DetailRow(label: 'Block', value: blockNumber.toString()),
              _DetailRow(label: 'Chain ID', value: chainId.toString()),

              const SizedBox(height: 20),

              // View on explorer button
              SizedBox(
                width: double.infinity,
                child: ElevatedButton.icon(
                  onPressed: () => _openBlockExplorer(chainId, txHash),
                  icon: const Icon(Icons.open_in_new, size: 18),
                  label: const Text('View on Block Explorer'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: CwColors.accent,
                    foregroundColor: Colors.white,
                    padding: const EdgeInsets.symmetric(vertical: 14),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(12),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  void _openBlockExplorer(int chainId, String txHash) {
    String baseUrl;
    switch (chainId) {
      case 1:
        baseUrl = 'https://etherscan.io';
        break;
      case 8453:
        baseUrl = 'https://basescan.org';
        break;
      case 84532:
        baseUrl = 'https://sepolia.basescan.org';
        break;
      case 42161:
        baseUrl = 'https://arbiscan.io';
        break;
      case 10:
        baseUrl = 'https://optimistic.etherscan.io';
        break;
      case 56:
        baseUrl = 'https://bscscan.com';
        break;
      default:
        baseUrl = 'https://etherscan.io';
    }

    final url = '$baseUrl/tx/$txHash';
    Clipboard.setData(ClipboardData(text: url));
  }
}

class _DetailRow extends StatelessWidget {
  final String label;
  final String value;
  final bool copyable;

  const _DetailRow({
    required this.label,
    required this.value,
    this.copyable = false,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 16),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 80,
            child: Text(
              label,
              style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w500,
                color: CwColors.ink3,
              ),
            ),
          ),
          Expanded(
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    value,
                    style: const TextStyle(
                      fontFamily: 'JetBrainsMono',
                      fontSize: 12,
                      color: CwColors.ink1,
                    ),
                  ),
                ),
                if (copyable)
                  IconButton(
                    icon: const Icon(Icons.copy, size: 16),
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(),
                    onPressed: () {
                      Clipboard.setData(ClipboardData(text: value));
                      showTopToast(context, 'Copied to clipboard');
                    },
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
