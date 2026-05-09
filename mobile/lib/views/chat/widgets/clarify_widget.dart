import 'package:flutter/material.dart';
import '../../../theme/colors.dart';

class ChatClarifyWidget extends StatelessWidget {
  final String question;
  final List<ClarifyOption> options;
  final bool resolved;
  final ValueChanged<String>? onSelect;

  const ChatClarifyWidget({
    super.key,
    required this.question,
    required this.options,
    this.resolved = false,
    this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(
          color: resolved ? CwColors.line : CwColors.accent.withValues(alpha: 0.3),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                resolved ? Icons.check_circle : Icons.help_outline,
                size: 16,
                color: resolved ? CwColors.ink4 : CwColors.accent,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  question,
                  style: TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w500,
                    color: resolved ? CwColors.ink3 : CwColors.ink1,
                    height: 1.4,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: options.map((opt) => _buildOption(opt)).toList(),
          ),
        ],
      ),
    );
  }

  Widget _buildOption(ClarifyOption opt) {
    return GestureDetector(
      onTap: resolved ? null : () => onSelect?.call(opt.prompt),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: resolved
              ? CwColors.bgPaper
              : CwColors.accent.withValues(alpha: 0.08),
          borderRadius: BorderRadius.circular(20),
          border: Border.all(
            color: resolved ? CwColors.line : CwColors.accent.withValues(alpha: 0.3),
          ),
        ),
        child: Text(
          opt.label,
          style: TextStyle(
            fontSize: 13,
            color: resolved ? CwColors.ink4 : CwColors.accent,
            fontWeight: FontWeight.w500,
          ),
        ),
      ),
    );
  }
}

class ClarifyOption {
  final String label;
  final String prompt;

  const ClarifyOption({required this.label, required this.prompt});

  factory ClarifyOption.fromJson(Map<String, dynamic> json) => ClarifyOption(
        label: json['label'] as String? ?? '',
        prompt: json['prompt'] as String? ?? '',
      );
}
