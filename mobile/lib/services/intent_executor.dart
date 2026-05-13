import '../l10n/strings.dart';
import '../models/tx_record.dart';
import 'action_result.dart';
import 'balance_service.dart';
import 'chain_service.dart';
import 'gas_service.dart';
import 'locator.dart';
import 'tx_history_service.dart';
import 'tx_service.dart';
import 'wallet_service.dart';

class IntentExecutor {
  final WalletService _wallet;
  final BalanceService _balance;
  final TxService _tx;
  final GasService _gas;
  final TxHistoryService _txHistory;
  final ChainService _chain;

  IntentExecutor({
    required WalletService wallet,
    required BalanceService balance,
    required TxService tx,
    required GasService gas,
    required TxHistoryService txHistory,
    required ChainService chain,
  })  : _wallet = wallet,
        _balance = balance,
        _tx = tx,
        _gas = gas,
        _txHistory = txHistory,
        _chain = chain;

  Future<ActionResult> execute(
    String kind,
    Map<String, String> params,
  ) async {
    // Block execution when emergency freeze is active
    if (Services.settings.emergencyFreezeActive) {
      return ActionResult.fail(
        S.lang == Lang.zh
            ? '紧急冻结已激活，所有操作已暂停。请先在设置中解除冻结。'
            : 'Emergency freeze is active. All operations paused. Deactivate in Settings first.',
      );
    }

    switch (kind) {
      case 'balance':
        return _executeBalance();
      case 'transfer':
        return _executeTransfer(params);
      case 'swap':
        return _executeSwap(params);
      default:
        return ActionResult.ok(
          S.lang == Lang.zh ? '好,这就办。' : 'On it.',
        );
    }
  }

  Future<ActionResult> _executeBalance() async {
    try {
      final address = await _wallet.getAddress();
      if (address.isEmpty) {
        return ActionResult.fail(S.noWallet);
      }
      await _balance.refresh(address);
      if (_balance.error != null) {
        return ActionResult.fail(
          '${S.balanceQueryFailed}: ${_balance.error}',
        );
      }
      return ActionResult.ok(
        S.lang == Lang.zh
            ? '你的余额: ${_balance.formattedEth} + ${_balance.formattedUsdc}'
            : 'Your balance: ${_balance.formattedEth} + ${_balance.formattedUsdc}',
        data: {
          'eth': _balance.formattedEth,
          'usdc': _balance.formattedUsdc,
          'total': _balance.formattedTotal,
        },
      );
    } catch (e) {
      return ActionResult.fail(
        S.lang == Lang.zh ? '出错了: $e' : 'Error: $e',
      );
    }
  }

  Future<ActionResult> estimateTransferGas(Map<String, String> params) async {
    try {
      final address = await _wallet.getAddress();
      if (address.isEmpty) return ActionResult.fail(S.noWallet);

      final to = params['to'] ?? '';
      final amountStr = params['amount'] ?? '0';
      final token = params['token'] ?? 'ETH';
      final amount = _parseAmount(amountStr, token);

      final estimate = await _gas.estimate(
        from: address,
        to: to,
        value: token == 'ETH' ? amount : BigInt.zero,
      );
      return ActionResult.ok(
        estimate.formattedUsd,
        data: {'gas': estimate.formattedUsd, 'gasEth': estimate.formattedEth},
      );
    } catch (e) {
      return ActionResult.fail(
        S.lang == Lang.zh ? 'Gas 估算失败' : 'Gas estimation failed',
      );
    }
  }

  Future<ActionResult> _executeTransfer(Map<String, String> params) async {
    try {
      final to = params['to'] ?? '';
      final amountStr = params['amount'] ?? '0';
      final token = (params['token'] ?? 'ETH').toUpperCase();
      final chainIdStr = params['chain_id'];
      final chainId = chainIdStr != null ? int.tryParse(chainIdStr) : null;
      final sendAll = params['send_all'] == 'true';

      if (to.isEmpty || !to.startsWith('0x') || to.length != 42) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '无效的收款地址' : 'Invalid recipient address',
        );
      }

      // Check balance before attempting transfer
      final address = await _wallet.getAddress();
      if (address.isEmpty) return ActionResult.fail(S.noWallet);

      // Switch chain RPC for balance/gas queries if needed
      final targetChainId = chainId ?? _resolveChainId(token);
      if (_chain is JsonRpcChainService) {
        (_chain as JsonRpcChainService).switchChain(ChainConfig.byId(targetChainId));
      }

      final isNativeToken = _isNativeToken(token, targetChainId);

      BigInt amount;
      if (sendAll && isNativeToken) {
        final balance = await _chain.getEthBalance(address);
        final baseFee = await _chain.getBaseFee() ?? await _chain.getGasPrice();
        final maxPriority = await _chain.getMaxPriorityFeePerGas();
        final maxFee = baseFee * BigInt.two + maxPriority;
        final gasCost = maxFee * BigInt.from(21000);
        amount = balance - gasCost;
        if (amount <= BigInt.zero) {
          return ActionResult.fail(
            S.lang == Lang.zh ? '余额不足以支付Gas费' : 'Insufficient balance for gas',
          );
        }
      } else {
        amount = _parseAmount(amountStr, token);
      }

      if (amount == BigInt.zero) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '无效的金额' : 'Invalid amount',
        );
      }

      final String txHash;

      if (isNativeToken) {
        txHash = await _tx.signAndSend(to: to, value: amount, chainId: targetChainId);
      } else {
        // ERC-20 token transfer (USDC, USDT, etc.)
        final config = ChainConfig.byId(targetChainId);
        final tokenContract = config.tokenContract(token);
        if (tokenContract.isEmpty) {
          return ActionResult.fail(
            S.lang == Lang.zh
                ? '不支持的代币: $token'
                : 'Unsupported token: $token',
          );
        }
        txHash = await _tx.sendErc20(
          to: to,
          tokenContract: tokenContract,
          amount: amount,
          chainId: targetChainId,
        );
      }

      // Record transaction locally
      await _txHistory.add(TxRecord(
        txHash: txHash,
        toAddress: to,
        value: amount,
        token: token,
        timestamp: DateTime.now(),
      ));

      // Show local notification for confirmed transaction
      Services.notifications.showTxConfirmed(txHash, amountStr, token);

      final shortHash =
          '${txHash.substring(0, 10)}...${txHash.substring(txHash.length - 6)}';
      return ActionResult.ok(
        S.lang == Lang.zh
            ? '转账成功! 交易: $shortHash'
            : 'Transfer sent! Tx: $shortHash',
        data: {'txHash': txHash, 'amount': amountStr, 'token': token},
      );
    } catch (e) {
      final msg = e.toString();
      final txHash = params['to'] ?? 'unknown';

      if (msg.contains('Biometric')) {
        Services.notifications.showTxFailed(txHash, S.bioAuthFailed);
        return ActionResult.fail(
          S.lang == Lang.zh
              ? '生物认证失败,转账已取消'
              : 'Biometric auth failed, transfer cancelled',
        );
      }
      if (msg.contains('insufficient funds') || msg.contains('InsufficientFunds')) {
        Services.notifications.showTxFailed(txHash, S.lang == Lang.zh ? '余额不足' : 'Insufficient balance');
        return ActionResult.fail(
          S.lang == Lang.zh ? '余额不足' : 'Insufficient balance',
        );
      }
      Services.notifications.showTxFailed(txHash, msg);
      return ActionResult.fail(
        S.lang == Lang.zh ? '转账失败: $msg' : 'Transfer failed: $msg',
      );
    }
  }

  Future<ActionResult> _executeSwap(Map<String, String> params) async {
    try {
      final fromToken = (params['from_token'] ?? '').toUpperCase();
      final toToken = (params['to_token'] ?? '').toUpperCase();
      final amountStr = params['amount'] ?? '0';
      if (fromToken.isEmpty || toToken.isEmpty) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '请指定兑换的代币' : 'Please specify swap tokens',
        );
      }

      final amount = _parseAmount(amountStr, fromToken);
      if (amount == BigInt.zero) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '无效的金额' : 'Invalid amount',
        );
      }

      // Swap is not yet implemented on-chain — return informative message
      return ActionResult.fail(
        S.lang == Lang.zh
            ? 'DEX 兑换功能开发中，暂时请使用外部 DEX 完成 $fromToken → $toToken 兑换'
            : 'DEX swap is under development. Please use an external DEX for $fromToken → $toToken swap.',
      );
    } catch (e) {
      return ActionResult.fail(
        S.lang == Lang.zh ? '兑换失败: $e' : 'Swap failed: $e',
      );
    }
  }

  bool _isNativeToken(String token, int chainId) {
    switch (token) {
      case 'ETH':
        return chainId == 1 || chainId == 8453 || chainId == 42161 || chainId == 10;
      case 'POL':
      case 'MATIC':
        return chainId == 137;
      case 'BNB':
        return chainId == 56;
      default:
        return false;
    }
  }

  int _resolveChainId(String token) {
    switch (token) {
      case 'POL':
      case 'MATIC':
        return 137;
      case 'BNB':
        return 56;
      case 'ETH':
      default:
        return 8453;
    }
  }

  BigInt _parseAmount(String input, String token) {
    final value = double.tryParse(input) ?? 0;
    if (token == 'ETH' || token == 'POL' || token == 'MATIC' || token == 'BNB') {
      return BigInt.from(value * 1e18);
    }
    return BigInt.from(value * 1e6); // USDC/USDT 6 decimals
  }
}
