import 'package:flutter/material.dart';
import '../../../theme/colors.dart';

class AddContactWidget extends StatelessWidget {
  final String name;
  final String address;
  final String? chain;
  final String? note;
  final bool loading;
  final bool resolved;
  final VoidCallback? onConfirm;
  final VoidCallback? onDeny;

  const AddContactWidget({
    super.key,
    required this.name,
    required this.address,
    this.chain,
    this.note,
    this.loading = false,
    this.resolved = false,
    this.onConfirm,
    this.onDeny,
  });

  @override
  Widget build(BuildContext context) {
    final shortAddr = address.length >= 10
        ? '${address.substring(0, 6)}...${address.substring(address.length - 4)}'
        : address;

    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  color: CwColors.accentSoft,
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Icon(Icons.person_add_alt_1, size: 20, color: CwColors.accent),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      name,
                      style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w600, color: CwColors.ink1),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      shortAddr,
                      style: TextStyle(fontSize: 13, color: CwColors.ink3, fontFamily: 'monospace'),
                    ),
                  ],
                ),
              ),
              if (resolved)
                Icon(Icons.check_circle, color: CwColors.success, size: 24),
            ],
          ),
          if (chain != null) ...[
            const SizedBox(height: 8),
            Row(
              children: [
                Icon(Icons.link, size: 14, color: CwColors.ink4),
                const SizedBox(width: 4),
                Text(chain!, style: TextStyle(fontSize: 12, color: CwColors.ink3)),
              ],
            ),
          ],
          if (note != null && note!.isNotEmpty) ...[
            const SizedBox(height: 4),
            Row(
              children: [
                Icon(Icons.note_outlined, size: 14, color: CwColors.ink4),
                const SizedBox(width: 4),
                Expanded(
                  child: Text(note!, style: TextStyle(fontSize: 12, color: CwColors.ink3)),
                ),
              ],
            ),
          ],
          if (!resolved) ...[
            const SizedBox(height: 14),
            Row(
              children: [
                Expanded(
                  child: OutlinedButton(
                    onPressed: loading ? null : onDeny,
                    child: const Text('取消'),
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: FilledButton(
                    onPressed: loading ? null : onConfirm,
                    child: loading
                        ? const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
                          )
                        : const Text('保存联系人'),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}
