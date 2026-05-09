import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../models/contact.dart';

class ContactsService extends ChangeNotifier {
  static const _storageKey = 'contacts_list';

  List<Contact> _contacts = [];
  List<Contact> get contacts => List.unmodifiable(_contacts);

  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    final json = prefs.getString(_storageKey);
    if (json != null && json.isNotEmpty) {
      _contacts = Contact.listFromJson(json);
      _contacts.sort((a, b) => b.createdAt.compareTo(a.createdAt));
      notifyListeners();
    }
  }

  Future<List<Contact>> getAll() async {
    if (_contacts.isEmpty) await load();
    return List.unmodifiable(_contacts);
  }

  Future<void> add(Contact contact) async {
    _contacts.insert(0, contact);
    await _save();
    notifyListeners();
  }

  Future<void> update(Contact contact) async {
    final index = _contacts.indexWhere((c) => c.id == contact.id);
    if (index >= 0) {
      _contacts[index] = contact;
      await _save();
      notifyListeners();
    }
  }

  Future<void> delete(String id) async {
    _contacts.removeWhere((c) => c.id == id);
    await _save();
    notifyListeners();
  }

  List<Contact> search(String query) {
    if (query.isEmpty) return List.unmodifiable(_contacts);
    final q = query.toLowerCase();
    return _contacts
        .where((c) =>
            c.name.toLowerCase().contains(q) ||
            c.address.toLowerCase().contains(q) ||
            (c.note?.toLowerCase().contains(q) ?? false))
        .toList();
  }

  Future<void> _save() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_storageKey, Contact.listToJson(_contacts));
  }
}
