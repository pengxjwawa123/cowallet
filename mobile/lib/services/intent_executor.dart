import '../api/swap_api.dart';
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
      final chain = _chain;
      if (chain is JsonRpcChainService) {
        chain.switchChain(ChainConfig.byId(targetChainId));
      }

      final isNativeToken = _isNativeToken(token, targetChainId);

      BigInt amount;
      final bool confirmedDeduct = params['confirmed_deduct'] == 'true';
      if (sendAll && isNativeToken) {
        final balance = await _chain.getEthBalance(address);
        final baseFee = await _chain.getBaseFee() ?? await _chain.getGasPrice();
        final maxPriority = await _chain.getMaxPriorityFeePerGas();
        final maxFee = baseFee + (baseFee ~/ BigInt.from(5)) + maxPriority;
        final gasCost = maxFee * BigInt.from(21000);
        amount = balance - gasCost;
        if (amount <= BigInt.zero) {
          return ActionResult.fail(
            S.lang == Lang.zh ? '余额不足以支付Gas费' : 'Insufficient balance for gas',
          );
        }
        if (!confirmedDeduct) {
          final nativeSymbol = (targetChainId == 137 || targetChainId == 80002) ? 'POL'
              : targetChainId == 56 ? 'BNB' : 'ETH';
          final balanceDisplay = _formatWei(balance, token);
          final maxSendableDisplay = _formatWei(amount, token);
          final gasCostDisplay = _formatWei(gasCost, token);
          return ActionResult.fail(
            S.lang == Lang.zh
                ? '转出全部余额需扣除Gas费。余额 $balanceDisplay $nativeSymbol，扣除Gas费后实际转出 $maxSendableDisplay $nativeSymbol，是否继续？'
                : 'Sending all requires gas deduction. Balance: $balanceDisplay $nativeSymbol, actual send: $maxSendableDisplay $nativeSymbol after gas. Continue?',
            data: {'suggest_deduct_gas': 'true', 'max_sendable': maxSendableDisplay, 'gas_cost': gasCostDisplay, 'symbol': nativeSymbol, 'original_amount': balanceDisplay},
          );
        }
      } else if (sendAll && !isNativeToken) {
        // ERC-20 send all: transfer the entire token balance (gas paid in native coin)
        final tokenInfo = _findTokenInBalance(token, targetChainId);
        final tokenContract = tokenInfo?.contractAddress
            ?? ChainConfig.byId(targetChainId).tokenContract(token);
        if (tokenContract.isEmpty) {
          return ActionResult.fail(
            S.lang == Lang.zh
                ? '未找到代币 $token 的合约地址'
                : 'Contract address for $token not found',
          );
        }
        amount = await _chain.getTokenBalance(address, tokenContract);
        if (amount <= BigInt.zero) {
          return ActionResult.fail(
            S.lang == Lang.zh ? '$token 余额为零' : '$token balance is zero',
          );
        }
      } else {
        amount = _parseAmount(amountStr, token, chainId: targetChainId);
      }

      if (amount == BigInt.zero) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '无效的金额' : 'Invalid amount',
        );
      }

      // Pre-check: verify sufficient balance before signing (skip for sendAll — already validated)
      if (isNativeToken && !sendAll) {
        final balance = await _chain.getEthBalance(address);
        final baseFee = await _chain.getBaseFee() ?? await _chain.getGasPrice();
        final maxPriority = await _chain.getMaxPriorityFeePerGas();
        final maxFee = baseFee + (baseFee ~/ BigInt.from(5)) + maxPriority;
        final gasCost = maxFee * BigInt.from(21000);
        if (balance < amount + gasCost) {
          final nativeSymbol = (targetChainId == 137 || targetChainId == 80002) ? 'POL'
              : targetChainId == 56 ? 'BNB' : 'ETH';
          final maxSendable = balance - gasCost;
          if (maxSendable > BigInt.zero) {
            final maxSendableDisplay = _formatWei(maxSendable, token);
            final gasCostDisplay = _formatWei(gasCost, token);
            return ActionResult.fail(
              S.lang == Lang.zh
                  ? '余额不足以支付转账金额+Gas费。扣除Gas费后最多可转出 $maxSendableDisplay $nativeSymbol (Gas≈$gasCostDisplay $nativeSymbol)，是否继续？'
                  : 'Insufficient balance for amount + gas. Max sendable after gas: $maxSendableDisplay $nativeSymbol (gas≈$gasCostDisplay $nativeSymbol). Continue?',
              data: {'suggest_deduct_gas': 'true', 'max_sendable': maxSendableDisplay, 'gas_cost': gasCostDisplay, 'symbol': nativeSymbol},
            );
          }
          return ActionResult.fail(
            S.lang == Lang.zh ? '余额不足以支付Gas费' : 'Insufficient balance for gas',
          );
        }
      }

      final String txHash;

      if (isNativeToken) {
        txHash = await _tx.signAndSend(to: to, value: amount, chainId: targetChainId);
      } else {
        // ERC-20: resolve contract address from user's balance data first
        final tokenInfo = _findTokenInBalance(token, targetChainId);
        final tokenContract = tokenInfo?.contractAddress
            ?? ChainConfig.byId(targetChainId).tokenContract(token);
        if (tokenContract.isEmpty) {
          return ActionResult.fail(
            S.lang == Lang.zh
                ? '未找到代币 $token 的合约地址，请确认你持有该代币'
                : 'Contract address for $token not found. Make sure you hold this token.',
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

      if (msg.contains('authentication') || msg.contains('Biometric')) {
        Services.notifications.showTxFailed(txHash, S.authFailed);
        return ActionResult.fail(
          S.lang == Lang.zh
              ? '身份验证失败，转账已取消'
              : 'Authentication failed, transfer cancelled',
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
      final chainIdStr = params['chain_id'];
      final slippageStr = params['slippage'];

      if (fromToken.isEmpty || toToken.isEmpty) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '请指定兑换的代币' : 'Please specify swap tokens',
        );
      }

      final swapChainId = chainIdStr != null
          ? (int.tryParse(chainIdStr) ?? _resolveChainId(fromToken))
          : _resolveChainId(fromToken);

      final amount = _parseAmount(amountStr, fromToken, chainId: swapChainId);
      if (amount == BigInt.zero) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '无效的金额' : 'Invalid amount',
        );
      }

      final address = await _wallet.getAddress();
      if (address.isEmpty) return ActionResult.fail(S.noWallet);

      final chainId = swapChainId;
      final slippage = slippageStr != null
          ? (double.tryParse(slippageStr) ?? 0.5)
          : 0.5;

      // Switch chain RPC for balance pre-check
      final chainSvc = _chain;
      if (chainSvc is JsonRpcChainService) {
        chainSvc.switchChain(ChainConfig.byId(chainId));
      }

      // Pre-check: verify sufficient balance for native token sells
      if (_isNativeToken(fromToken, chainId)) {
        final balance = await _chain.getEthBalance(address);
        if (balance < amount) {
          return ActionResult.fail(
            S.lang == Lang.zh ? '余额不足' : 'Insufficient balance',
          );
        }
      } else {
        final config = ChainConfig.byId(chainId);
        final tokenContract = config.tokenContract(fromToken);
        if (tokenContract.isNotEmpty) {
          final tokenBalance = await _chain.getTokenBalance(address, tokenContract);
          if (tokenBalance < amount) {
            return ActionResult.fail(
              S.lang == Lang.zh ? '$fromToken 余额不足' : 'Insufficient $fromToken balance',
            );
          }
        }
      }

      // Build swap transaction via backend (0x aggregator)
      final buildResult = await SwapApi.buildSwapTx(
        chainId: chainId,
        sellToken: fromToken,
        buyToken: toToken,
        sellAmount: amountStr,
        takerAddress: address,
        slippage: slippage,
      );

      if (!buildResult.isSuccess || buildResult.data == null) {
        return ActionResult.fail(
          S.lang == Lang.zh
              ? '获取兑换路由失败: ${buildResult.errorMessage ?? "未知错误"}'
              : 'Failed to get swap route: ${buildResult.errorMessage ?? "unknown error"}',
        );
      }

      final swapData = buildResult.data!;
      final swapTo = swapData['to'] as String? ?? '';
      final swapCalldata = swapData['data'] as String? ?? '';
      final swapValue = swapData['value'] as String? ?? '0';
      final gasEstimateStr = swapData['gas_estimate'] as String? ?? '200000';
      final buyAmount = swapData['buy_amount'] as String? ?? '';

      if (swapTo.isEmpty || swapCalldata.isEmpty) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '兑换交易数据无效' : 'Invalid swap transaction data',
        );
      }

      // Parse the value to send with the swap (for native token sells)
      final swapValueBigInt = BigInt.tryParse(
        swapValue.startsWith('0x')
            ? swapValue.substring(2)
            : swapValue,
        radix: swapValue.startsWith('0x') ? 16 : 10,
      ) ?? BigInt.zero;

      final gasLimit = BigInt.tryParse(gasEstimateStr) ?? BigInt.from(200000);

      // TODO: If selling ERC-20, may need approval tx first.
      // The 0x API's allowance_target field tells us which contract needs approval.
      // For now, the backend handles this check and returns an error if approval is needed.

      // Sign and send the swap transaction
      final txHash = await _tx.signAndSend(
        to: swapTo,
        value: swapValueBigInt,
        data: swapCalldata,
        gasLimit: gasLimit,
        chainId: chainId,
      );

      // Record transaction locally
      await _txHistory.add(TxRecord(
        txHash: txHash,
        toAddress: swapTo,
        value: amount,
        token: '$fromToken>$toToken',
        timestamp: DateTime.now(),
      ));

      // Notification
      Services.notifications.showTxConfirmed(txHash, amountStr, '$fromToken>$toToken');

      final shortHash =
          '${txHash.substring(0, 10)}...${txHash.substring(txHash.length - 6)}';

      // Format buy amount for display
      final buyDisplayAmount = _formatBuyAmount(buyAmount, toToken);

      return ActionResult.ok(
        S.lang == Lang.zh
            ? '兑换成功! $amountStr $fromToken → $buyDisplayAmount $toToken\n交易: $shortHash'
            : 'Swap successful! $amountStr $fromToken → $buyDisplayAmount $toToken\nTx: $shortHash',
        data: {
          'txHash': txHash,
          'fromToken': fromToken,
          'toToken': toToken,
          'sellAmount': amountStr,
          'buyAmount': buyDisplayAmount,
        },
      );
    } catch (e) {
      final msg = e.toString();
      if (msg.contains('authentication') || msg.contains('Biometric')) {
        return ActionResult.fail(
          S.lang == Lang.zh
              ? '身份验证失败，兑换已取消'
              : 'Authentication failed, swap cancelled',
        );
      }
      if (msg.contains('insufficient funds') || msg.contains('InsufficientFunds')) {
        return ActionResult.fail(
          S.lang == Lang.zh ? '余额不足' : 'Insufficient balance',
        );
      }
      if (msg.contains('allowance') || msg.contains('ALLOWANCE')) {
        return ActionResult.fail(
          S.lang == Lang.zh
              ? '需要先授权代币额度，请稍后重试'
              : 'Token approval required. Please try again shortly.',
        );
      }
      return ActionResult.fail(
        S.lang == Lang.zh ? '兑换失败: $msg' : 'Swap failed: $msg',
      );
    }
  }

  /// Format raw buy amount from the DEX response into human-readable form.
  String _formatBuyAmount(String rawAmount, String token) {
    if (rawAmount.isEmpty) return '~';
    // If it looks like a decimal already (from backend formatting), return as-is
    if (rawAmount.contains('.')) return rawAmount;
    // Otherwise parse as raw integer and format
    final raw = BigInt.tryParse(rawAmount);
    if (raw == null) return rawAmount;
    return _formatWei(raw, token.isEmpty ? 'ETH' : token);
  }

  bool _isNativeToken(String token, int chainId) {
    final t = token.toUpperCase();
    // Known native symbols for specific chains
    if (t == 'POL' || t == 'MATIC') return chainId == 137 || chainId == 80002;
    if (t == 'BNB') return chainId == 56;
    if (t == 'ETH') {
      // ETH is native on EVM L1/L2s (not Polygon/BSC)
      // But if AI mistakenly says "ETH" on Polygon/BSC, treat as native transfer
      // because it's clearly a native coin transfer intent (no contract given)
      return true;
    }
    return false;
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
        return _chain.currentConfig.chainId;
    }
  }

  /// Look up token from user's balance data by symbol and chain.
  TokenBalance? _findTokenInBalance(String token, int chainId) {
    final chainTokens = _balance.tokensForChain(chainId);
    final match = chainTokens.where(
      (t) => t.symbol.toUpperCase() == token.toUpperCase() && !t.native,
    );
    return match.isEmpty ? null : match.first;
  }

  BigInt _parseAmount(String input, String token, {int? chainId}) {
    final value = double.tryParse(input) ?? 0;
    final decimals = _resolveDecimals(token, chainId);
    if (decimals >= 18) {
      return BigInt.from(value * 1e18);
    } else if (decimals >= 8) {
      return BigInt.from(value * 1e8);
    }
    return BigInt.from(value * 1e6);
  }

  int _resolveDecimals(String token, int? chainId) {
    // Try balance data first (authoritative, from chain)
    if (chainId != null) {
      final info = _findTokenInBalance(token, chainId);
      if (info != null) return info.decimals;
    }
    // Fallback to known defaults
    switch (token.toUpperCase()) {
      case 'USDC':
      case 'USDT':
        return 6;
      case 'WBTC':
        return 8;
      default:
        return 18;
    }
  }

  String _formatWei(BigInt wei, String token, {int? chainId}) {
    final decimals = _resolveDecimals(token, chainId);
    final divisor = BigInt.from(10).pow(decimals);
    final whole = wei ~/ divisor;
    final frac = wei.remainder(divisor).abs();
    final fracStr = frac.toString().padLeft(decimals, '0');
    final showDigits = decimals < 6 ? decimals : 6;
    final trimmed = fracStr.substring(0, showDigits).replaceAll(RegExp(r'0+$'), '');
    if (trimmed.isEmpty) return whole.toString();
    return '$whole.$trimmed';
  }
}
