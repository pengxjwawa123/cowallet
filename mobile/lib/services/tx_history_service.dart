import 'package:flutter/foundation.dart';

import '../models/tx_record.dart';
import '../platform/secure_storage.dart';
import 'chain_service.dart';

class TxHistoryService extends ChangeNotifier {
  final SecureStorageService _storage;
  final ChainService _chain;
  static const _storageKey = 'tx_history';

  List<TxRecord> _records = [];
  List<TxRecord> get records => List.unmodifiable(_records);

  TxHistoryService({
    required SecureStorageService storage,
    required ChainService chain,
  })  : _storage = storage,
        _chain = chain;

  Future<void> load() async {
    final json = await _storage.read(_storageKey);
    if (json != null && json.isNotEmpty) {
      _records = TxRecord.listFromJson(json);
      notifyListeners();
    }
  }

  Future<void> add(TxRecord record) async {
    _records.insert(0, record);
    if (_records.length > 50) _records = _records.sublist(0, 50);
    await _save();
    notifyListeners();
  }

  Future<void> refreshStatuses() async {
    var changed = false;
    for (final record in _records) {
      if (record.status != TxStatus.pending) continue;
      try {
        final receipt = await _chain.getTransactionReceipt(record.txHash);
        if (receipt != null) {
          final statusHex = receipt['status'] as String?;
          record.status =
              statusHex == '0x1' ? TxStatus.confirmed : TxStatus.failed;
          final blockHex = receipt['blockNumber'] as String?;
          if (blockHex != null) {
            record.blockNumber = int.parse(blockHex);
          }
          changed = true;
        }
      } catch (_) {
        // RPC failure — skip, retry next time
      }
    }
    if (changed) {
      await _save();
      notifyListeners();
    }
  }

  Future<void> _save() async {
    await _storage.write(_storageKey, TxRecord.listToJson(_records));
  }
}
