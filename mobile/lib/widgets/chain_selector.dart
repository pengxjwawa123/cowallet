import 'package:flutter/material.dart';
import '../config/api_config.dart';
import '../l10n/strings.dart';
import '../theme/colors.dart';
import '../main.dart';

/// A tappable chip showing the current chain name.
/// On tap, shows a bottom sheet with all available chains grouped by mainnet/testnet.
class ChainSelector extends StatelessWidget {
  const ChainSelector({super.key});

  @override
  Widget build(BuildContext context) {
    final appState = CowalletApp.of(context);
    // Force rebuild via ListenableBuilder so chip updates on chain switch
    return ListenableBuilder(
      listenable: appState,
      builder: (context, _) => _buildChip(context, appState),
    );
  }

  Widget _buildChip(BuildContext context, appState) {
    final chain = appState.selectedChain;

    return GestureDetector(
      onTap: () => _showChainSheet(context),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
        decoration: BoxDecoration(
          color: CwColors.bgCard,
          borderRadius: BorderRadius.circular(20),
          border: Border.all(color: CwColors.line),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            _chainDot(chain),
            const SizedBox(width: 6),
            Text(
              chain.displayName,
              style: const TextStyle(
                fontFamily: 'Inter',
                fontSize: 12,
                fontWeight: FontWeight.w600,
                color: CwColors.ink2,
              ),
            ),
            const SizedBox(width: 4),
            const Icon(Icons.keyboard_arrow_down_rounded, size: 16, color: CwColors.ink3),
          ],
        ),
      ),
    );
  }

  void _showChainSheet(BuildContext context) {
    final appState = CowalletApp.of(context);

    showModalBottomSheet(
      context: context,
      backgroundColor: CwColors.bgPaper,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (sheetContext) => _ChainList(
        selectedChainId: appState.selectedChain.chainId,
        onSelect: (chain) {
          appState.setChain(chain);
          Navigator.pop(sheetContext);
        },
      ),
    );
  }

  static Widget _chainDot(ChainConfig chain) {
    final color = _chainColor(chain);
    return Container(
      width: 8,
      height: 8,
      decoration: BoxDecoration(
        color: color,
        shape: BoxShape.circle,
      ),
    );
  }

  static Color _chainColor(ChainConfig chain) {
    switch (chain.name) {
      case 'ethereum':
      case 'sepolia':
        return const Color(0xFF627EEA);
      case 'base':
      case 'base-sepolia':
        return const Color(0xFF0052FF);
      case 'arbitrum':
        return const Color(0xFF28A0F0);
      case 'optimism':
        return const Color(0xFFFF0420);
      case 'bsc':
        return const Color(0xFFF3BA2F);
      case 'polygon':
        return const Color(0xFF8247E5);
      default:
        return CwColors.ink3;
    }
  }
}

class _ChainList extends StatelessWidget {
  final int selectedChainId;
  final ValueChanged<ChainConfig> onSelect;

  const _ChainList({
    required this.selectedChainId,
    required this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.of(context).size.height * 0.6,
        ),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 16, 20, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Handle bar
              Center(
                child: Container(
                  width: 36,
                  height: 4,
                  decoration: BoxDecoration(
                    color: CwColors.lineStrong,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),
              const SizedBox(height: 16),

              // Title
              Text(
                S.selectNetwork,
                style: const TextStyle(
                  fontFamily: 'NotoSerifSC',
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: CwColors.ink1,
                ),
              ),
              const SizedBox(height: 16),

              // Scrollable chain list
              Flexible(
                child: SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      _sectionHeader(S.mainnets),
                      const SizedBox(height: 8),
                      ...ChainConfig.supportedMainnets.map((c) => _chainTile(c)),
                      const SizedBox(height: 16),
                      _sectionHeader(S.testnets),
                      const SizedBox(height: 8),
                      ...ChainConfig.supportedTestnets.map((c) => _chainTile(c)),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _sectionHeader(String title) {
    return Text(
      title,
      style: const TextStyle(
        fontFamily: 'JetBrainsMono',
        fontSize: 10,
        fontWeight: FontWeight.w600,
        letterSpacing: 0.8,
        color: CwColors.ink3,
      ),
    );
  }

  Widget _chainTile(ChainConfig chain) {
    final isSelected = chain.chainId == selectedChainId;

    return GestureDetector(
      onTap: () => onSelect(chain),
      child: Container(
        margin: const EdgeInsets.only(bottom: 4),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: isSelected ? CwColors.accentSoft2 : Colors.transparent,
          borderRadius: BorderRadius.circular(12),
          border: isSelected
              ? Border.all(color: CwColors.accent.withValues(alpha: 0.3))
              : null,
        ),
        child: Row(
          children: [
            // Chain color dot
            Container(
              width: 10,
              height: 10,
              decoration: BoxDecoration(
                color: ChainSelector._chainColor(chain),
                shape: BoxShape.circle,
              ),
            ),
            const SizedBox(width: 12),

            // Name + symbol
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    chain.displayName,
                    style: TextStyle(
                      fontFamily: 'Inter',
                      fontSize: 14,
                      fontWeight: isSelected ? FontWeight.w700 : FontWeight.w500,
                      color: CwColors.ink1,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    chain.symbol,
                    style: const TextStyle(
                      fontFamily: 'JetBrainsMono',
                      fontSize: 11,
                      color: CwColors.ink3,
                    ),
                  ),
                ],
              ),
            ),

            // Checkmark
            if (isSelected)
              const Icon(Icons.check_rounded, size: 18, color: CwColors.accent),
          ],
        ),
      ),
    );
  }
}
