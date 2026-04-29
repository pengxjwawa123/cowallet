import 'package:flutter/material.dart';
import '../theme/colors.dart';

enum ChipVariant { green, amber, accent, info, danger }

class CwChip extends StatelessWidget {
  final String label;
  final ChipVariant variant;
  final bool showDot;
  final double fontSize;

  const CwChip({
    super.key,
    required this.label,
    this.variant = ChipVariant.green,
    this.showDot = false,
    this.fontSize = 11,
  });

  @override
  Widget build(BuildContext context) {
    final (bg, fg) = switch (variant) {
      ChipVariant.green => (CwColors.successSoft, CwColors.success),
      ChipVariant.amber => (CwColors.warnSoft, CwColors.warn),
      ChipVariant.accent => (CwColors.accentSoft, CwColors.accent),
      ChipVariant.info => (CwColors.infoSoft, CwColors.info),
      ChipVariant.danger => (CwColors.dangerSoft, CwColors.danger),
    };

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(6),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (showDot) ...[
            Container(
              width: 6,
              height: 6,
              decoration: BoxDecoration(color: fg, shape: BoxShape.circle),
            ),
            const SizedBox(width: 4),
          ],
          Text(
            label,
            style: TextStyle(
              fontSize: fontSize,
              fontWeight: FontWeight.w600,
              color: fg,
            ),
          ),
        ],
      ),
    );
  }
}
