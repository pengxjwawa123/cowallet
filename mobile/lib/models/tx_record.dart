import 'dart:convert';

enum TxStatus { pending, confirmed, failed }

class TxRecord {
  final String txHash;
  final String toAddress;
  final BigInt value;
  final String token;
  final DateTime timestamp;
  TxStatus status;
  int? blockNumber;

  TxRecord({
    required this.txHash,
    required this.toAddress,
    required this.value,
    required this.token,
    required this.timestamp,
    this.status = TxStatus.pending,
    this.blockNumber,
  });

  Map<String, dynamic> toJson() => {
        'txHash': txHash,
        'toAddress': toAddress,
        'value': value.toString(),
        'token': token,
        'timestamp': timestamp.toIso8601String(),
        'status': status.name,
        'blockNumber': blockNumber,
      };

  factory TxRecord.fromJson(Map<String, dynamic> json) => TxRecord(
        txHash: json['txHash'] as String,
        toAddress: json['toAddress'] as String,
        value: BigInt.parse(json['value'] as String),
        token: json['token'] as String,
        timestamp: DateTime.parse(json['timestamp'] as String),
        status: TxStatus.values.byName(json['status'] as String),
        blockNumber: json['blockNumber'] as int?,
      );

  static List<TxRecord> listFromJson(String jsonStr) {
    final list = jsonDecode(jsonStr) as List;
    return list
        .map((e) => TxRecord.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  static String listToJson(List<TxRecord> records) =>
      jsonEncode(records.map((r) => r.toJson()).toList());
}
