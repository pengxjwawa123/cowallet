import 'package:flutter/material.dart';
import '../theme/colors.dart';

class IntentCard extends StatelessWidget {
  final String headerLabel;
  final String title;
  final String subtitle;
  final String confirmLabel;
  final String denyLabel;
  final VoidCallback? onConfirm;
  final VoidCallback? onDeny;
  final bool loading;

  const IntentCard({
    super.key,
    required this.headerLabel,
    required this.title,
    required this.subtitle,
    required this.confirmLabel,
    required this.denyLabel,
    this.onConfirm,
    this.onDeny,
    this.loading = false,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(top: 10),
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        border: Border.all(color: CwColors.accent, width: 2),
        borderRadius: BorderRadius.circular(14),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 8,
                height: 8,
                decoration: BoxDecoration(
                  color: CwColors.accent,
                  shape: BoxShape.circle,
                  boxShadow: [
                    BoxShadow(
                      color: CwColors.accentSoft,
                      blurRadius: 6,
                      spreadRadius: 2,
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 6),
              Text(
                headerLabel,
                style: const TextStyle(
                  fontFamily: 'JetBrainsMono',
                  fontSize: 10,
                  letterSpacing: 1.0,
                  fontWeight: FontWeight.w600,
                  color: CwColors.accentHover,
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Text(
            title,
            style: const TextStyle(
              fontFamily: 'NotoSerifSC',
              fontSize: 15,
              fontWeight: FontWeight.w500,
              height: 1.35,
              color: CwColors.ink1,
            ),
          ),
          const SizedBox(height: 6),
          Text(
            subtitle,
            style: const TextStyle(fontSize: 13, color: CwColors.ink2, height: 1.55),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: FilledButton(
                  onPressed: loading ? null : onConfirm,
                  style: FilledButton.styleFrom(
                    minimumSize: const Size.fromHeight(40),
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                    textStyle: const TextStyle(fontSize: 12.5, fontWeight: FontWeight.w600),
                  ),
                  child: loading
                      ? const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: Colors.white,
                          ),
                        )
                      : Text(confirmLabel),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: OutlinedButton(
                  onPressed: onDeny,
                  style: OutlinedButton.styleFrom(
                    minimumSize: const Size.fromHeight(40),
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                    textStyle: const TextStyle(fontSize: 12.5),
                  ),
                  child: Text(denyLabel),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
