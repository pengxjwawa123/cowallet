import 'dart:convert';

class Contact {
  final String id;
  final String name;
  final String address;
  final String? chain;
  final String? note;
  final DateTime createdAt;

  Contact({
    required this.id,
    required this.name,
    required this.address,
    this.chain,
    this.note,
    required this.createdAt,
  });

  Contact copyWith({
    String? name,
    String? address,
    String? chain,
    String? note,
  }) =>
      Contact(
        id: id,
        name: name ?? this.name,
        address: address ?? this.address,
        chain: chain ?? this.chain,
        note: note ?? this.note,
        createdAt: createdAt,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'address': address,
        'chain': chain,
        'note': note,
        'createdAt': createdAt.toIso8601String(),
      };

  factory Contact.fromJson(Map<String, dynamic> json) => Contact(
        id: json['id'] as String,
        name: json['name'] as String,
        address: json['address'] as String,
        chain: json['chain'] as String?,
        note: json['note'] as String?,
        createdAt: DateTime.parse(json['createdAt'] as String),
      );

  static List<Contact> listFromJson(String jsonStr) {
    final list = jsonDecode(jsonStr) as List;
    return list
        .map((e) => Contact.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  static String listToJson(List<Contact> contacts) =>
      jsonEncode(contacts.map((c) => c.toJson()).toList());
}
