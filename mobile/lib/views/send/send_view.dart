import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../models/tx_record.dart';
import '../../services/locator.dart';
import '../../services/gas_service.dart';

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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(S.enterAddress)),
      );
      return;
    }
    if (amountNum == null || amountNum <= 0) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(S.enterValidAmount)),
      );
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

    setState(() => _sending = true);
    try {
      final txHash = await Services.tx.signAndSend(
        to: to,
        value: _parseAmount(amountText),
        gasLimit: _gasEstimate?.gasLimit,
        maxFeePerGas: _gasEstimate?.maxFeePerGas,
        maxPriorityFeePerGas: _gasEstimate?.maxPriorityFeePerGas,
      );

      await Services.txHistory.add(TxRecord(
        txHash: txHash,
        toAddress: to,
        value: _parseAmount(amountText),
        token: _selectedToken,
        timestamp: DateTime.now(),
      ));

      if (!mounted) return;
      await showDialog<void>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: Text(S.txSuccess),
          content: Text('${S.txHashLabel}:\n$txHash'),
          actions: [
            FilledButton(
              onPressed: () => Navigator.pop(ctx),
              child: Text(S.done),
            ),
          ],
        ),
      );
      if (mounted) Navigator.pop(context);
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('${S.txFailed}: $e')),
      );
    } finally {
      if (mounted) setState(() => _sending = false);
    }
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
    return Container(
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
            child: Text('Base',
                style: Theme.of(context).textTheme.labelLarge),
          ),
        ],
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
