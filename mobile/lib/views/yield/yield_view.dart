import 'package:flutter/material.dart';
import '../../api/yield_api.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/section_label.dart';
import '../../widgets/cw_chip.dart';

class YieldView extends StatefulWidget {
  const YieldView({super.key});

  @override
  State<YieldView> createState() => _YieldViewState();
}

class _YieldViewState extends State<YieldView> {
  List<YieldOpportunity> _opportunities = [];
  bool _loading = true;
  String? _error;
  double _bestApy = 0;
  double _averageApy = 0;
  String? _selectedType;

  @override
  void initState() {
    super.initState();
    _loadOpportunities();
  }

  Future<void> _loadOpportunities() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    final result = await YieldApi.search(
      protocolType: _selectedType,
      limit: 20,
    );

    if (!mounted) return;

    if (result.isSuccess && result.data != null) {
      final data = result.data!;
      final opps = (data['opportunities'] as List<dynamic>?)
              ?.map((e) => YieldOpportunity.fromJson(e as Map<String, dynamic>))
              .toList() ??
          [];
      setState(() {
        _opportunities = opps;
        _bestApy = (data['best_apy'] ?? 0).toDouble();
        _averageApy = (data['average_apy'] ?? 0).toDouble();
        _loading = false;
      });
    } else {
      setState(() {
        _error = result.errorMessage ?? S.yieldLoadFailed;
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final tt = Theme.of(context).textTheme;

    return RefreshIndicator(
      color: CwColors.accent,
      onRefresh: _loadOpportunities,
      child: ListView(
        padding: const EdgeInsets.symmetric(horizontal: 20),
        children: [
          const SizedBox(height: 12),

          // Header card
          _headerCard(tt),
          const SizedBox(height: 8),

          // Filter chips
          SectionLabel(title: S.yieldOpportunities),
          _filterChips(),
          const SizedBox(height: 12),

          // Content
          if (_loading)
            _loadingState()
          else if (_error != null)
            _errorState()
          else if (_opportunities.isEmpty)
            _emptyState(tt)
          else
            ..._opportunities.map((opp) => Padding(
                  padding: const EdgeInsets.only(bottom: 10),
                  child: _opportunityCard(context, opp),
                )),

          const SizedBox(height: 32),
        ],
      ),
    );
  }

  Widget _headerCard(TextTheme tt) {
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        gradient: const LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [Color(0xFFE1ECD9), Color(0xFFD9E8D0)],
        ),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            S.yieldLabel,
            style: const TextStyle(
              fontFamily: 'JetBrainsMono',
              fontSize: 11,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.5,
              color: CwColors.ink3,
            ),
          ),
          const SizedBox(height: 8),
          Text(
            S.yieldH1,
            style: tt.headlineLarge,
          ),
          const SizedBox(height: 6),
          Text(
            S.yieldSub,
            style: tt.bodyMedium?.copyWith(color: CwColors.ink3),
          ),
          const SizedBox(height: 14),
          // APY summary row
          if (!_loading && _bestApy > 0)
            Row(
              children: [
                _apySummaryPill(S.yieldBestApy, '${_bestApy.toStringAsFixed(1)}%'),
                const SizedBox(width: 10),
                _apySummaryPill(S.yieldAvgApy, '${_averageApy.toStringAsFixed(1)}%'),
              ],
            ),
        ],
      ),
    );
  }

  Widget _apySummaryPill(String label, String value) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.7),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            label,
            style: const TextStyle(
              fontSize: 11,
              color: CwColors.ink3,
            ),
          ),
          const SizedBox(width: 4),
          Text(
            value,
            style: const TextStyle(
              fontFamily: 'JetBrainsMono',
              fontSize: 12,
              fontWeight: FontWeight.w700,
              color: CwColors.success,
            ),
          ),
        ],
      ),
    );
  }

  Widget _filterChips() {
    final filters = [
      (null, S.yieldAll),
      ('lending', S.yieldLending),
      ('liquid_staking', S.yieldStaking),
      ('dex', 'DEX'),
      ('vault', S.yieldVault),
      ('farm', S.yieldFarm),
    ];

    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: filters.map((f) {
          final isSelected = _selectedType == f.$1;
          return Padding(
            padding: const EdgeInsets.only(right: 8),
            child: GestureDetector(
              onTap: () {
                setState(() => _selectedType = f.$1);
                _loadOpportunities();
              },
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                decoration: BoxDecoration(
                  color: isSelected ? CwColors.ink1 : CwColors.bgCard,
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(
                    color: isSelected ? CwColors.ink1 : CwColors.line,
                  ),
                ),
                child: Text(
                  f.$2,
                  style: TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w500,
                    color: isSelected ? Colors.white : CwColors.ink2,
                  ),
                ),
              ),
            ),
          );
        }).toList(),
      ),
    );
  }

  Widget _opportunityCard(BuildContext context, YieldOpportunity opp) {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Top row: protocol + type badge + APY
          Row(
            children: [
              // Protocol icon circle
              Container(
                width: 36,
                height: 36,
                decoration: BoxDecoration(
                  color: _protocolColor(opp.opportunityType),
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Center(
                  child: Text(
                    _protocolEmoji(opp.opportunityType),
                    style: const TextStyle(fontSize: 18),
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      opp.protocolName,
                      style: Theme.of(context).textTheme.titleMedium?.copyWith(
                            fontWeight: FontWeight.w600,
                            color: CwColors.ink1,
                          ),
                    ),
                    const SizedBox(height: 2),
                    Row(
                      children: [
                        Flexible(
                          child: Text(
                            opp.tokenLabel,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(
                              fontFamily: 'JetBrainsMono',
                              fontSize: 11,
                              color: CwColors.ink3,
                            ),
                          ),
                        ),
                        const SizedBox(width: 6),
                        CwChip(
                          label: opp.typeLabel,
                          variant: ChipVariant.info,
                          fontSize: 9,
                        ),
                      ],
                    ),
                  ],
                ),
              ),
              // APY highlight
              Column(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Text(
                    '${opp.apy.toStringAsFixed(1)}%',
                    style: const TextStyle(
                      fontFamily: 'JetBrainsMono',
                      fontSize: 18,
                      fontWeight: FontWeight.w700,
                      color: CwColors.success,
                    ),
                  ),
                  const Text(
                    'APY',
                    style: TextStyle(
                      fontSize: 10,
                      color: CwColors.ink3,
                    ),
                  ),
                ],
              ),
            ],
          ),

          const SizedBox(height: 12),

          // Bottom row: TVL + risk + deposit button
          Row(
            children: [
              // TVL
              Text(
                'TVL ${_formatUsd(opp.tvlUsd)}',
                style: const TextStyle(
                  fontFamily: 'JetBrainsMono',
                  fontSize: 11,
                  fontWeight: FontWeight.w500,
                  color: CwColors.ink3,
                ),
              ),
              const SizedBox(width: 10),
              // Risk level
              CwChip(
                label: _riskLabel(opp.riskLevel),
                variant: _riskVariant(opp.riskLevel),
                fontSize: 10,
              ),
              if (opp.lockDays != null) ...[
                const SizedBox(width: 6),
                CwChip(
                  label: '${opp.lockDays}d lock',
                  variant: ChipVariant.amber,
                  fontSize: 10,
                ),
              ],
              const Spacer(),
              // Deposit button
              GestureDetector(
                onTap: () => _showStrategySheet(context, opp),
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                  decoration: BoxDecoration(
                    color: CwColors.ink1,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    S.yieldDeposit,
                    style: const TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w600,
                      color: Colors.white,
                    ),
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  void _showStrategySheet(BuildContext context, YieldOpportunity opp) {
    showModalBottomSheet(
      context: context,
      backgroundColor: CwColors.bgPaper,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (ctx) => Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Text(
                  opp.protocolName,
                  style: Theme.of(context).textTheme.titleLarge?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                ),
                const Spacer(),
                Text(
                  '${opp.apy.toStringAsFixed(1)}% APY',
                  style: const TextStyle(
                    fontFamily: 'JetBrainsMono',
                    fontSize: 16,
                    fontWeight: FontWeight.w700,
                    color: CwColors.success,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 6),
            Text(
              opp.tokenLabel,
              style: const TextStyle(
                fontFamily: 'JetBrainsMono',
                fontSize: 13,
                color: CwColors.ink3,
              ),
            ),
            if (opp.strategy != null) ...[
              const SizedBox(height: 16),
              Text(
                S.yieldStrategy,
                style: const TextStyle(
                  fontFamily: 'JetBrainsMono',
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: CwColors.ink3,
                ),
              ),
              const SizedBox(height: 6),
              Text(
                opp.strategy!,
                style: const TextStyle(
                  fontSize: 13,
                  color: CwColors.ink2,
                  height: 1.5,
                ),
              ),
            ],
            const SizedBox(height: 16),
            // APY breakdown
            Text(
              S.yieldApyBreakdown,
              style: const TextStyle(
                fontFamily: 'JetBrainsMono',
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: CwColors.ink3,
              ),
            ),
            const SizedBox(height: 8),
            _breakdownRow(S.yieldBaseApy, opp.apyBreakdown.baseApy),
            if (opp.apyBreakdown.rewardApy > 0)
              _breakdownRow(S.yieldRewardApy, opp.apyBreakdown.rewardApy),
            if (opp.apyBreakdown.incentiveApy > 0)
              _breakdownRow(S.yieldIncentiveApy, opp.apyBreakdown.incentiveApy),
            const SizedBox(height: 16),
            // Risk factors
            if (opp.riskFactors.isNotEmpty) ...[
              Text(
                S.yieldRisks,
                style: const TextStyle(
                  fontFamily: 'JetBrainsMono',
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: CwColors.ink3,
                ),
              ),
              const SizedBox(height: 6),
              Wrap(
                spacing: 6,
                runSpacing: 6,
                children: opp.riskFactors
                    .map((r) => CwChip(
                          label: r,
                          variant: ChipVariant.amber,
                          fontSize: 10,
                        ))
                    .toList(),
              ),
            ],
            const SizedBox(height: 24),
            // Deposit CTA
            SizedBox(
              width: double.infinity,
              height: 48,
              child: ElevatedButton(
                onPressed: () {
                  Navigator.pop(ctx);
                  // TODO: navigate to deposit flow
                },
                style: ElevatedButton.styleFrom(
                  backgroundColor: CwColors.ink1,
                  foregroundColor: Colors.white,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(14),
                  ),
                ),
                child: Text(
                  S.yieldDepositNow,
                  style: const TextStyle(
                    fontWeight: FontWeight.w600,
                    fontSize: 15,
                  ),
                ),
              ),
            ),
            const SizedBox(height: 8),
          ],
        ),
      ),
    );
  }

  Widget _breakdownRow(String label, double value) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        children: [
          Text(
            label,
            style: const TextStyle(fontSize: 12, color: CwColors.ink3),
          ),
          const Spacer(),
          Text(
            '${value.toStringAsFixed(2)}%',
            style: const TextStyle(
              fontFamily: 'JetBrainsMono',
              fontSize: 12,
              fontWeight: FontWeight.w600,
              color: CwColors.ink2,
            ),
          ),
        ],
      ),
    );
  }

  Widget _loadingState() {
    return const Padding(
      padding: EdgeInsets.symmetric(vertical: 60),
      child: Center(
        child: CircularProgressIndicator(
          strokeWidth: 2,
          color: CwColors.accent,
        ),
      ),
    );
  }

  Widget _errorState() {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 40),
      child: Center(
        child: Column(
          children: [
            Text(
              _error ?? S.yieldLoadFailed,
              style: const TextStyle(color: CwColors.ink3, fontSize: 13),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 12),
            GestureDetector(
              onTap: _loadOpportunities,
              child: Text(
                S.retry,
                style: const TextStyle(
                  color: CwColors.accent,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _emptyState(TextTheme tt) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 40),
      child: Center(
        child: Column(
          children: [
            const Text(
              '📊',
              style: TextStyle(fontSize: 40),
            ),
            const SizedBox(height: 12),
            Text(
              S.yieldEmpty,
              style: tt.bodyMedium?.copyWith(color: CwColors.ink3),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }

  // Helpers

  String _formatUsd(double value) {
    if (value >= 1e9) return '\$${(value / 1e9).toStringAsFixed(1)}B';
    if (value >= 1e6) return '\$${(value / 1e6).toStringAsFixed(0)}M';
    if (value >= 1e3) return '\$${(value / 1e3).toStringAsFixed(0)}K';
    return '\$${value.toStringAsFixed(0)}';
  }

  Color _protocolColor(String type) {
    switch (type) {
      case 'lending':
        return CwColors.infoSoft;
      case 'liquid_staking':
        return CwColors.successSoft;
      case 'dex':
        return CwColors.accentSoft;
      case 'vault':
        return CwColors.warnSoft;
      case 'farm':
        return CwColors.goldSoft;
      default:
        return CwColors.bgSubtle;
    }
  }

  String _protocolEmoji(String type) {
    switch (type) {
      case 'lending':
        return '🏦';
      case 'liquid_staking':
        return '⚡';
      case 'dex':
        return '🔄';
      case 'vault':
        return '🏰';
      case 'farm':
        return '🌾';
      default:
        return '💰';
    }
  }

  String _riskLabel(String level) {
    switch (level) {
      case 'low':
        return S.yieldRiskLow;
      case 'medium':
        return S.yieldRiskMed;
      case 'high':
        return S.yieldRiskHigh;
      case 'very_high':
        return S.yieldRiskVeryHigh;
      default:
        return level;
    }
  }

  ChipVariant _riskVariant(String level) {
    switch (level) {
      case 'low':
        return ChipVariant.green;
      case 'medium':
        return ChipVariant.amber;
      case 'high':
        return ChipVariant.danger;
      case 'very_high':
        return ChipVariant.danger;
      default:
        return ChipVariant.amber;
    }
  }
}
