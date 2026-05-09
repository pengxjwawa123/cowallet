import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../l10n/strings.dart';
import '../../models/contact.dart';
import '../../services/locator.dart';
import '../../theme/colors.dart';
import 'add_contact_sheet.dart';

class ContactsView extends StatefulWidget {
  const ContactsView({super.key});

  @override
  State<ContactsView> createState() => _ContactsViewState();
}

class _ContactsViewState extends State<ContactsView> {
  final _searchCtrl = TextEditingController();
  String _query = '';

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  List<Contact> get _filtered => Services.contacts.search(_query);

  Future<void> _addContact() async {
    final result = await showModalBottomSheet<Contact>(
      context: context,
      isScrollControlled: true,
      backgroundColor: CwColors.bgPaper,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (_) => const AddContactSheet(),
    );
    if (result != null) {
      await Services.contacts.add(result);
    }
  }

  Future<void> _editContact(Contact contact) async {
    final result = await showModalBottomSheet<Contact>(
      context: context,
      isScrollControlled: true,
      backgroundColor: CwColors.bgPaper,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (_) => AddContactSheet(existing: contact),
    );
    if (result != null) {
      await Services.contacts.update(result);
    }
  }

  Future<void> _deleteContact(Contact contact) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(S.contactsDelete),
        content: Text(S.contactsDeleteConfirm),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: Text(S.cancel),
          ),
          TextButton(
            onPressed: () => Navigator.pop(ctx, true),
            style: TextButton.styleFrom(foregroundColor: CwColors.danger),
            child: Text(S.confirm),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      await Services.contacts.delete(contact.id);
    }
  }

  void _copyAddress(Contact contact) {
    Clipboard.setData(ClipboardData(text: contact.address));
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(S.addressCopied),
        duration: const Duration(seconds: 2),
      ),
    );
  }

  void _showContactActions(Contact contact) {
    showModalBottomSheet(
      context: context,
      backgroundColor: CwColors.bgPaper,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (ctx) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 12),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              // Handle bar
              Container(
                width: 36,
                height: 4,
                decoration: BoxDecoration(
                  color: CwColors.ink4,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
              const SizedBox(height: 12),
              ListTile(
                leading: const Icon(Icons.copy, color: CwColors.ink2),
                title: Text(S.copyAddress),
                onTap: () {
                  Navigator.pop(ctx);
                  _copyAddress(contact);
                },
              ),
              ListTile(
                leading: const Icon(Icons.edit_outlined, color: CwColors.ink2),
                title: Text(S.contactsEdit),
                onTap: () {
                  Navigator.pop(ctx);
                  _editContact(contact);
                },
              ),
              ListTile(
                leading: const Icon(Icons.delete_outline, color: CwColors.danger),
                title: Text(
                  S.contactsDelete,
                  style: const TextStyle(color: CwColors.danger),
                ),
                onTap: () {
                  Navigator.pop(ctx);
                  _deleteContact(contact);
                },
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
      backgroundColor: CwColors.bgPaper,
      appBar: AppBar(
        title: Text(S.contactsTitle),
        backgroundColor: CwColors.bgPaper,
        elevation: 0,
        scrolledUnderElevation: 0,
        foregroundColor: CwColors.ink1,
        actions: [
          IconButton(
            icon: const Icon(Icons.add, color: CwColors.accent),
            onPressed: _addContact,
          ),
        ],
      ),
      body: Column(
        children: [
          // Search bar
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 4, 16, 12),
            child: TextField(
              controller: _searchCtrl,
              onChanged: (v) => setState(() => _query = v),
              decoration: InputDecoration(
                hintText: S.contactsSearch,
                hintStyle: const TextStyle(color: CwColors.ink4),
                prefixIcon: const Icon(Icons.search, color: CwColors.ink4, size: 20),
                filled: true,
                fillColor: CwColors.bgCard,
                contentPadding: const EdgeInsets.symmetric(vertical: 10),
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
              ),
            ),
          ),
          // Contact list
          Expanded(
            child: ListenableBuilder(
              listenable: Services.contacts,
              builder: (context, _) {
                final list = _filtered;
                if (list.isEmpty) {
                  return Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        const Icon(
                          Icons.people_outline,
                          size: 64,
                          color: CwColors.ink4,
                        ),
                        const SizedBox(height: 16),
                        Text(
                          S.contactsEmpty,
                          style: Theme.of(context)
                              .textTheme
                              .bodyMedium
                              ?.copyWith(color: CwColors.ink3),
                        ),
                        const SizedBox(height: 12),
                        TextButton.icon(
                          onPressed: _addContact,
                          icon: const Icon(Icons.add),
                          label: Text(S.contactsAdd),
                          style: TextButton.styleFrom(
                            foregroundColor: CwColors.accent,
                          ),
                        ),
                      ],
                    ),
                  );
                }
                return ListView.separated(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  itemCount: list.length,
                  separatorBuilder: (_, _) => const Divider(
                    height: 1,
                    indent: 56,
                    color: CwColors.line,
                  ),
                  itemBuilder: (context, index) {
                    final contact = list[index];
                    return _contactTile(contact);
                  },
                );
              },
            ),
          ),
        ],
      ),
    );
  }

  Widget _contactTile(Contact contact) {
    final shortAddr =
        '${contact.address.substring(0, 6)}...${contact.address.substring(contact.address.length - 4)}';
    return ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
      leading: CircleAvatar(
        backgroundColor: CwColors.accentSoft,
        child: Text(
          contact.name.isNotEmpty ? contact.name[0].toUpperCase() : '?',
          style: const TextStyle(
            color: CwColors.accent,
            fontWeight: FontWeight.w600,
          ),
        ),
      ),
      title: Text(
        contact.name,
        style: const TextStyle(
          color: CwColors.ink1,
          fontWeight: FontWeight.w500,
          fontSize: 15,
        ),
      ),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            shortAddr,
            style: const TextStyle(
              fontFamily: 'JetBrainsMono',
              fontSize: 12,
              color: CwColors.ink3,
            ),
          ),
          if (contact.note != null && contact.note!.isNotEmpty)
            Text(
              contact.note!,
              style: const TextStyle(fontSize: 11, color: CwColors.ink4),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
        ],
      ),
      trailing: const Icon(Icons.more_vert, color: CwColors.ink4, size: 20),
      onTap: () => _copyAddress(contact),
      onLongPress: () => _showContactActions(contact),
    );
  }
}
