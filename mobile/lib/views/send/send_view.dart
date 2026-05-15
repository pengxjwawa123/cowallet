import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../models/tx_record.dart';
import '../../services/locator.dart';
import '../../services/gas_service.dart';
import '../../services/policy_service.dart';
import '../../services/chain_service.dart' as rpc;
import '../../config/api_config.dart';
import '../../main.dart';
import '../../widgets/top_toast.dart';
import 'tx_tracking_view.dart';

class SendView extends StatefulWidget {
  const SendView({super.key});

  @override
  State<SendView> createState() => _SendViewState();
}

class _SendViewState extends State<SendView> {
  final _addressController = TextEditingController();
  final _amountController = TextEditingController();
  String _selectedToken = 'ETH';
  bool _sending = false;
  GasEstimate? _gasEstimate;
  bool _gasLoading = false;
  String? _gasError;

  @override
  void initState() {
    super.initState();
    _amountController.addListener(_onAmountChanged);
    _addressController.addListener(_onAmountChanged);
  }

  @override
  void dispose() {
    _addressController.dispose();
    _amountController.dispose();
    super.dispose();
  }

  void _onAmountChanged() {
    _debounceGas();
  }

  int _gasDebounceId = 0;
  void _debounceGas() {
    _gasDebounceId++;
    final id = _gasDebounceId;
    Future.delayed(const Duration(milliseconds: 500), () {
      if (id == _gasDebounceId && mounted) _fetchGas();
    });
  }

  Future<void> _fetchGas() async {
    final to = _addressController.text.trim();
    final amountText = _amountController.text.trim();
    final amount = double.tryParse(amountText);
    if (to.isEmpty || amount == null || amount <= 0) {
      setState(() {
        _gasEstimate = null;
        _gasError = null;
        _gasLoading = false;
      });
      return;
    }

    setState(() {
      _gasLoading = true;
      _gasError = null;
    });

    try {
      final address = await Services.wallet.getAddress();
      final estimate = await Services.gas.estimate(
        from: address,
        to: to,
        value: _selectedToken == 'ETH' ? _parseAmount(amountText) : BigInt.zero,
      );
      if (!mounted) return;
      setState(() {
        _gasEstimate = estimate;
        _gasLoading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _gasEstimate = null;
        _gasError = S.gasEstimateFailed;
        _gasLoading = false;
      });
    }
  }

  BigInt _parseAmount(String input) {
    final value = double.tryParse(input) ?? 0;
    if (_selectedToken == 'ETH') {
      return BigInt.from(value * 1e18);
    }
    return BigInt.from(value * 1e6);
  }

  Future<void> _confirmAndSend() async {
    final to = _addressController.text.trim();
    final amountText = _amountController.text.trim();
    final amountNum = double.tryParse(amountText);

    if (to.isEmpty) {
      showTopToast(context, S.enterAddress);
      return;
    }
    if (amountNum == null || amountNum <= 0) {
      showTopToast(context, S.enterValidAmount);
      return;
    }

    final gasDisplay = _gasEstimate?.formattedUsd ?? '~\$0.03';
    final shortTo = to.length >= 10
        ? '${to.substring(0, 6)}...${to.substring(to.length - 4)}'
        : to;

    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(S.confirmTransfer),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('${S.amountLabel}: $amountText $_selectedToken'),
            const SizedBox(height: 8),
            Text('${S.recipientLabel}: $shortTo'),
            const SizedBox(height: 8),
            Text('${S.estGas}: ~$gasDisplay'),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: Text(S.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: Text(S.confirmTransfer),
          ),
        ],
      ),
    );

    if (confirmed != true || !mounted) return;

    // Biometric authentication gate before MPC signing
    final bioOk = await Services.biometrics.authenticate(
      reason: S.biometricAuthReason,
    );
    if (!bioOk) {
      if (mounted) {
        showTopToast(context, S.bioAuthFailed, backgroundColor: CwColors.danger);
      }
      return;
    }
    if (!mounted) return;

    // Policy engine check — verify tx is within configured limits/rules
    final policyResult = await _checkPolicy(to, amountText, amountNum);
    if (!mounted) return;

    if (policyResult.decision == PolicyDecision.deny) {
      _showPolicyDeniedDialog(policyResult);
      return;
    }

    if (policyResult.decision == PolicyDecision.requireApproval) {
      final approved = await _showPolicyApprovalSheet(policyResult);
      if (approved != true || !mounted) return;
    }

    setState(() => _sending = true);
    try {
      String txHash;
      if (_selectedToken == 'ETH') {
        txHash = await Services.tx.signAndSend(
          to: to,
          value: _parseAmount(amountText),
          gasLimit: _gasEstimate?.gasLimit,
          maxFeePerGas: _gasEstimate?.maxFeePerGas,
          maxPriorityFeePerGas: _gasEstimate?.maxPriorityFeePerGas,
        );
      } else {
        final tokenContract = Services.chain.tokenContract(_selectedToken);
        txHash = await Services.tx.sendErc20(
          to: to,
          tokenContract: tokenContract,
          amount: _parseAmount(amountText),
          gasLimit: _gasEstimate?.gasLimit,
          maxFeePerGas: _gasEstimate?.maxFeePerGas,
          maxPriorityFeePerGas: _gasEstimate?.maxPriorityFeePerGas,
        );
      }

      await Services.txHistory.add(TxRecord(
        txHash: txHash,
        toAddress: to,
        value: _parseAmount(amountText),
        token: _selectedToken,
        timestamp: DateTime.now(),
      ));

      if (!mounted) return;
      Navigator.pushReplacement(
        context,
        MaterialPageRoute(
          builder: (_) => TxTrackingView(
            txHash: txHash,
            toAddress: to,
            amount: amountText,
            token: _selectedToken,
          ),
        ),
      );
    } catch (e) {
      if (!mounted) return;
      showTopToast(context, '${S.txFailed}: $e', backgroundColor: CwColors.danger);
    } finally {
      if (mounted) setState(() => _sending = false);
    }
  }

  Future<PolicyCheckResult> _checkPolicy(
    String to,
    String amountText,
    double amountNum,
  ) async {
    final appState = CowalletApp.of(context);
    final chainId = appState.selectedChain.chainId;
    final address = await Services.wallet.getAddress();

    return Services.policy.checkTransaction(
      from: address,
      to: to,
      value: _parseAmount(amountText).toString(),
      token: _selectedToken,
      chainId: chainId,
      amountUsd: amountNum, // approximate; backend may re-price
    );
  }

  void _showPolicyDeniedDialog(PolicyCheckResult result) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        icon: const Icon(Icons.block_rounded, color: CwColors.danger, size: 40),
        title: Text(S.policyDeniedTitle),
        content: Text(
          result.reason ?? S.policyDeniedDefault,
          textAlign: TextAlign.center,
        ),
        actions: [
          FilledButton(
            onPressed: () => Navigator.pop(ctx),
            child: Text(S.policyOk),
          ),
        ],
      ),
    );
  }

  Future<bool?> _showPolicyApprovalSheet(PolicyCheckResult result) {
    return showModalBottomSheet<bool>(
      context: context,
      backgroundColor: CwColors.bgPaper,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (ctx) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              // Handle bar
              Center(
                child: Container(
                  width: 36,
                  height: 4,
                  decoration: BoxDecoration(
                    color: CwColors.lineStrong,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),
              const SizedBox(height: 20),
              const Icon(Icons.warning_amber_rounded,
                  color: CwColors.warn, size: 48),
              const SizedBox(height: 16),
              Text(
                S.policyApprovalTitle,
                style: const TextStyle(
                  fontSize: 18,
                  fontWeight: FontWeight.w700,
                  color: CwColors.ink1,
                ),
              ),
              const SizedBox(height: 12),
              Text(
                result.reason ?? S.policyApprovalDefault,
                textAlign: TextAlign.center,
                style: const TextStyle(
                  fontSize: 14,
                  color: CwColors.ink3,
                ),
              ),
              if (result.policyName != null) ...[
                const SizedBox(height: 8),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                    color: CwColors.bgSubtle,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    result.policyName!,
                    style: const TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                      color: CwColors.ink3,
                    ),
                  ),
                ),
              ],
              const SizedBox(height: 24),
              Row(
                children: [
                  Expanded(
                    child: OutlinedButton(
                      onPressed: () => Navigator.pop(ctx, false),
                      child: Text(S.cancel),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: FilledButton(
                      onPressed: () => Navigator.pop(ctx, true),
                      child: Text(S.policyApprovalProceed),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(S.sendTitle, style: Theme.of(context).textTheme.titleLarge),
      ),
      body: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _recipientField(context),
            const SizedBox(height: 16),
            _amountField(context),
            const SizedBox(height: 16),
            _chainInfo(context),
            const Spacer(),
            _feeEstimate(context),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: _sending ? null : _confirmAndSend,
              child: _sending
                  ? const SizedBox(
                      width: 20,
                      height: 20,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Text(S.confirmTransfer),
            ),
            const SizedBox(height: 8),
          ],
        ),
      ),
    );
  }

  Widget _recipientField(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(S.recipient, style: Theme.of(context).textTheme.bodySmall),
        const SizedBox(height: 6),
        TextField(
          controller: _addressController,
          decoration: InputDecoration(
            hintText: S.addressHint,
            hintStyle: TextStyle(color: CwColors.ink4),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: CwColors.line),
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: CwColors.line),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: CwColors.accent),
            ),
            suffixIcon: IconButton(
              icon: const Icon(Icons.qr_code_scanner, color: CwColors.ink4),
              onPressed: () {},
            ),
          ),
        ),
      ],
    );
  }

  Widget _amountField(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(S.amount, style: Theme.of(context).textTheme.bodySmall),
        const SizedBox(height: 6),
        TextField(
          controller: _amountController,
          keyboardType: const TextInputType.numberWithOptions(decimal: true),
          decoration: InputDecoration(
            hintText: '0.00',
            hintStyle: TextStyle(color: CwColors.ink4),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: CwColors.line),
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: CwColors.line),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: CwColors.accent),
            ),
            suffixIcon: Padding(
              padding: const EdgeInsets.only(right: 8),
              child: DropdownButtonHideUnderline(
                child: DropdownButton<String>(
                  value: _selectedToken,
                  items: ['ETH', 'USDC', 'USDT']
                      .map((t) => DropdownMenuItem(value: t, child: Text(t)))
                      .toList(),
                  onChanged: (v) {
                    setState(() => _selectedToken = v!);
                    _debounceGas();
                  },
                  style: Theme.of(context).textTheme.labelLarge,
                ),
              ),
            ),
          ),
        ),
        const SizedBox(height: 4),
        Align(
          alignment: Alignment.centerRight,
          child: ListenableBuilder(
            listenable: Services.balance,
            builder: (context, _) => Text(
              '${S.balancePrefix}: ${_selectedToken == 'ETH' ? Services.balance.formattedEth : Services.balance.formattedUsdc}',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        ),
      ],
    );
  }

  Widget _chainInfo(BuildContext context) {
    final appState = CowalletApp.of(context);
    return ListenableBuilder(
      listenable: appState,
      builder: (context, _) {
        final chain = appState.selectedChain;
        return GestureDetector(
          onTap: () => _showChainSelector(context),
          child: Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: CwColors.bgSubtle,
              borderRadius: BorderRadius.circular(12),
            ),
            child: Row(
              children: [
                const Icon(Icons.link, color: CwColors.ink3, size: 18),
                const SizedBox(width: 8),
                Text(S.network, style: Theme.of(context).textTheme.bodySmall),
                const Spacer(),
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                    color: CwColors.bgCard,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: CwColors.line),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(chain.displayName,
                          style: Theme.of(context).textTheme.labelLarge),
                      const SizedBox(width: 4),
                      const Icon(Icons.keyboard_arrow_down_rounded,
                          size: 16, color: CwColors.ink3),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  void _showChainSelector(BuildContext context) {
    final appState = CowalletApp.of(context);

    showModalBottomSheet(
      context: context,
      backgroundColor: CwColors.bgPaper,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (sheetContext) => _SendChainList(
        selectedChainId: appState.selectedChain.chainId,
        onSelect: (chain) {
          appState.setChain(chain);
          // Switch the RPC chain service to match
          final rpcConfig = rpc.ChainConfig.byId(chain.chainId);
          (Services.chain as rpc.JsonRpcChainService).switchChain(rpcConfig);
          // Invalidate gas price cache for the new chain
          Services.gas.clearCache();
          // Clear stale gas estimate and re-fetch for new chain
          setState(() {
            _gasEstimate = null;
            _gasError = null;
          });
          _gasDebounceId++;
          _fetchGas();
          Navigator.pop(sheetContext);
        },
      ),
    );
  }

  Widget _feeEstimate(BuildContext context) {
    String gasDisplay;
    if (_gasLoading) {
      gasDisplay = S.gasEstimating;
    } else if (_gasError != null) {
      gasDisplay = _gasError!;
    } else if (_gasEstimate != null) {
      gasDisplay = '~${_gasEstimate!.formattedUsd}';
    } else {
      gasDisplay = '—';
    }

    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: CwColors.bgSubtle,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        children: [
          _feeRow(context, S.estGas, gasDisplay),
          const SizedBox(height: 6),
          _feeRow(context, S.sigMethod, 'MPC 2-of-3'),
        ],
      ),
    );
  }

  Widget _feeRow(BuildContext context, String label, String value) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Text(label, style: Theme.of(context).textTheme.bodySmall),
        Text(value, style: Theme.of(context).textTheme.labelLarge),
      ],
    );
  }
}

/// Chain list bottom sheet for the send view.
/// Reuses the same visual style as ChainSelector's sheet.
class _SendChainList extends StatelessWidget {
  final int selectedChainId;
  final ValueChanged<ChainConfig> onSelect;

  const _SendChainList({
    required this.selectedChainId,
    required this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.of(context).size.height * 0.6,
        ),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 16, 20, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Handle bar
              Center(
                child: Container(
                  width: 36,
                  height: 4,
                  decoration: BoxDecoration(
                    color: CwColors.lineStrong,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),
              const SizedBox(height: 16),

              // Title
              Text(
                S.selectNetwork,
                style: const TextStyle(
                  fontFamily: 'NotoSerifSC',
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: CwColors.ink1,
                ),
              ),
              const SizedBox(height: 16),

              // Scrollable chain list
              Flexible(
                child: SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      _sectionHeader(S.mainnets),
                      const SizedBox(height: 8),
                      ...ChainConfig.supportedMainnets.map((c) => _chainTile(context, c)),
                      const SizedBox(height: 16),
                      _sectionHeader(S.testnets),
                      const SizedBox(height: 8),
                      ...ChainConfig.supportedTestnets.map((c) => _chainTile(context, c)),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _sectionHeader(String title) {
    return Text(
      title,
      style: const TextStyle(
        fontFamily: 'JetBrainsMono',
        fontSize: 10,
        fontWeight: FontWeight.w600,
        letterSpacing: 0.8,
        color: CwColors.ink3,
      ),
    );
  }

  Widget _chainTile(BuildContext context, ChainConfig chain) {
    final isSelected = chain.chainId == selectedChainId;

    return GestureDetector(
      onTap: () => onSelect(chain),
      child: Container(
        margin: const EdgeInsets.only(bottom: 4),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: isSelected ? CwColors.accentSoft2 : Colors.transparent,
          borderRadius: BorderRadius.circular(12),
          border: isSelected
              ? Border.all(color: CwColors.accent.withValues(alpha: 0.3))
              : null,
        ),
        child: Row(
          children: [
            // Chain color dot
            Container(
              width: 10,
              height: 10,
              decoration: BoxDecoration(
                color: _chainColor(chain),
                shape: BoxShape.circle,
              ),
            ),
            const SizedBox(width: 12),

            // Name + symbol
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    chain.displayName,
                    style: TextStyle(
                      fontFamily: 'Inter',
                      fontSize: 14,
                      fontWeight: isSelected ? FontWeight.w700 : FontWeight.w500,
                      color: CwColors.ink1,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    chain.symbol,
                    style: const TextStyle(
                      fontFamily: 'JetBrainsMono',
                      fontSize: 11,
                      color: CwColors.ink3,
                    ),
                  ),
                ],
              ),
            ),

            // Testnet badge
            if (chain.isTestnet)
              Container(
                margin: const EdgeInsets.only(right: 8),
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: CwColors.warn.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  S.testnetBadge,
                  style: const TextStyle(
                    fontSize: 9,
                    fontWeight: FontWeight.w600,
                    color: CwColors.warn,
                  ),
                ),
              ),

            // Checkmark
            if (isSelected)
              const Icon(Icons.check_rounded, size: 18, color: CwColors.accent),
          ],
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
}
