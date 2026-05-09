import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../agents/agents_view.dart';
import 'yield_view.dart';

/// Combined DeFi + Agents hub view with a top segment switcher.
class DefiHubView extends StatefulWidget {
  const DefiHubView({super.key});

  @override
  State<DefiHubView> createState() => _DefiHubViewState();
}

class _DefiHubViewState extends State<DefiHubView> {
  int _selected = 0; // 0 = Earn, 1 = Agents

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        // Top segment control
        SafeArea(
          bottom: false,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(20, 12, 20, 0),
            child: Container(
              height: 36,
              decoration: BoxDecoration(
                color: CwColors.bgSubtle,
                borderRadius: BorderRadius.circular(10),
              ),
              child: Row(
                children: [
                  _segmentButton(0, S.tabDefi),
                  _segmentButton(1, S.tabAgents),
                ],
              ),
            ),
          ),
        ),
        // Content
        Expanded(
          child: IndexedStack(
            index: _selected,
            children: const [
              YieldView(),
              AgentsView(),
            ],
          ),
        ),
      ],
    );
  }

  Widget _segmentButton(int index, String label) {
    final isActive = _selected == index;
    return Expanded(
      child: GestureDetector(
        onTap: () => setState(() => _selected = index),
        child: Container(
          margin: const EdgeInsets.all(3),
          decoration: BoxDecoration(
            color: isActive ? CwColors.bgCard : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
            boxShadow: isActive
                ? [
                    BoxShadow(
                      color: CwColors.ink1.withValues(alpha: 0.06),
                      blurRadius: 4,
                      offset: const Offset(0, 1),
                    ),
                  ]
                : null,
          ),
          child: Center(
            child: Text(
              label,
              style: TextStyle(
                fontSize: 13,
                fontWeight: isActive ? FontWeight.w600 : FontWeight.w500,
                color: isActive ? CwColors.ink1 : CwColors.ink3,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
