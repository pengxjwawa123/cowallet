import 'package:flutter/material.dart';

import '../../l10n/strings.dart';
import '../../models/contact.dart';
import '../../theme/colors.dart';

class AddContactSheet extends StatefulWidget {
  final Contact? existing;

  const AddContactSheet({super.key, this.existing});

  @override
  State<AddContactSheet> createState() => _AddContactSheetState();
}

class _AddContactSheetState extends State<AddContactSheet> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameCtrl;
  late final TextEditingController _addressCtrl;
  late final TextEditingController _noteCtrl;

  bool get _isEditing => widget.existing != null;

  @override
  void initState() {
    super.initState();
    _nameCtrl = TextEditingController(text: widget.existing?.name ?? '');
    _addressCtrl = TextEditingController(text: widget.existing?.address ?? '');
    _noteCtrl = TextEditingController(text: widget.existing?.note ?? '');
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _addressCtrl.dispose();
    _noteCtrl.dispose();
    super.dispose();
  }

  bool _isValidAddress(String value) {
    final trimmed = value.trim();
    if (trimmed.length != 42) return false;
    if (!trimmed.startsWith('0x')) return false;
    final hex = trimmed.substring(2);
    return RegExp(r'^[0-9a-fA-F]{40}$').hasMatch(hex);
  }

  void _submit() {
    if (!_formKey.currentState!.validate()) return;
    final now = DateTime.now();
    final contact = Contact(
      id: widget.existing?.id ??
          '${now.millisecondsSinceEpoch}_${_addressCtrl.text.trim().substring(2, 8)}',
      name: _nameCtrl.text.trim(),
      address: _addressCtrl.text.trim(),
      note: _noteCtrl.text.trim().isEmpty ? null : _noteCtrl.text.trim(),
      createdAt: widget.existing?.createdAt ?? now,
    );
    Navigator.of(context).pop(contact);
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.only(
        left: 20,
        right: 20,
        top: 20,
        bottom: MediaQuery.of(context).viewInsets.bottom + 20,
      ),
      child: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Handle bar
            Center(
              child: Container(
                width: 36,
                height: 4,
                decoration: BoxDecoration(
                  color: CwColors.ink4,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
            ),
            const SizedBox(height: 16),
            Text(
              _isEditing ? S.contactsEdit : S.contactsAdd,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 20),
            // Name field
            TextFormField(
              controller: _nameCtrl,
              decoration: InputDecoration(
                labelText: S.contactsName,
                hintText: S.contactsNameHint,
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
              validator: (value) {
                if (value == null || value.trim().isEmpty) {
                  return S.contactsNameRequired;
                }
                return null;
              },
            ),
            const SizedBox(height: 14),
            // Address field
            TextFormField(
              controller: _addressCtrl,
              decoration: InputDecoration(
                labelText: S.contactsAddress,
                hintText: '0x...',
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
              validator: (value) {
                if (value == null || value.trim().isEmpty) {
                  return S.contactsAddressRequired;
                }
                if (!_isValidAddress(value)) {
                  return S.contactsAddressInvalid;
                }
                return null;
              },
            ),
            const SizedBox(height: 14),
            // Note field
            TextFormField(
              controller: _noteCtrl,
              decoration: InputDecoration(
                labelText: S.contactsNote,
                hintText: S.contactsNoteHint,
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
              maxLines: 2,
            ),
            const SizedBox(height: 20),
            // Submit button
            SizedBox(
              height: 48,
              child: ElevatedButton(
                onPressed: _submit,
                style: ElevatedButton.styleFrom(
                  backgroundColor: CwColors.accent,
                  foregroundColor: Colors.white,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(12),
                  ),
                ),
                child: Text(
                  _isEditing ? S.contactsSave : S.contactsAdd,
                  style: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
