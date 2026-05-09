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

      if (to.isEmpty || !to.startsWith('0x') || to.length != 42) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '无效的收款地址' : 'Invalid recipient address',
        );
      }

      final amount = _parseAmount(amountStr, token);
      if (amount == BigInt.zero) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '无效的金额' : 'Invalid amount',
        );
      }

      // Check balance before attempting transfer
      final address = await _wallet.getAddress();
      if (address.isEmpty) return ActionResult.fail(S.noWallet);

      final String txHash;

      if (token == 'ETH') {
        // Native ETH transfer via MPC signing
        txHash = await _tx.signAndSend(to: to, value: amount);
      } else {
        // ERC-20 token transfer (USDC, USDT, etc.)
        final tokenContract = _chain.tokenContract(token);
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

  BigInt _parseAmount(String input, String token) {
    final value = double.tryParse(input) ?? 0;
    if (token == 'ETH') {
      return BigInt.from(value * 1e18);
    }
    return BigInt.from(value * 1e6); // USDC 6 decimals
  }
}
