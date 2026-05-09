import 'package:flutter/material.dart';
import '../../../theme/colors.dart';

class ChatAuditWidget extends StatelessWidget {
  final Map<String, dynamic> data;

  const ChatAuditWidget({super.key, required this.data});

  @override
  Widget build(BuildContext context) {
    final score = data['score'] as int? ?? 0;
    final riskLevel = data['risk_level'] as String? ?? 'unknown';
    final findings = (data['findings'] as List<dynamic>?) ?? [];
    final recommendations = (data['recommendations'] as List<dynamic>?) ?? [];

    final scoreColor = score >= 90
        ? CwColors.success
        : (score >= 70 ? const Color(0xFFE5A100) : CwColors.danger);

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
              const Icon(Icons.shield_outlined, size: 16, color: CwColors.accent),
              const SizedBox(width: 6),
              Text(
                '安全审计',
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: CwColors.ink3,
                  letterSpacing: 0.5,
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          // Score circle
          Center(
            child: Column(
              children: [
                Container(
                  width: 72,
                  height: 72,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    border: Border.all(color: scoreColor, width: 3),
                  ),
                  child: Center(
                    child: Text(
                      '$score',
                      style: TextStyle(
                        fontSize: 28,
                        fontWeight: FontWeight.w700,
                        fontFamily: 'JetBrainsMono',
                        color: scoreColor,
                      ),
                    ),
                  ),
                ),
                const SizedBox(height: 6),
                Text(
                  _riskLabel(riskLevel),
                  style: TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w500,
                    color: scoreColor,
                  ),
                ),
              ],
            ),
          ),
          if (findings.isNotEmpty) ...[
            const SizedBox(height: 16),
            const Text(
              '检查项目',
              style: TextStyle(fontSize: 11, fontWeight: FontWeight.w600, color: CwColors.ink3),
            ),
            const SizedBox(height: 8),
            ...findings.map((f) => _buildFinding(f)).toList(),
          ],
          if (recommendations.isNotEmpty) ...[
            const SizedBox(height: 12),
            const Text(
              '建议',
              style: TextStyle(fontSize: 11, fontWeight: FontWeight.w600, color: CwColors.ink3),
            ),
            const SizedBox(height: 6),
            ...recommendations.map((r) => Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text('•  ', style: TextStyle(fontSize: 12, color: CwColors.ink3)),
                  Expanded(
                    child: Text(
                      r.toString(),
                      style: const TextStyle(fontSize: 12, color: CwColors.ink2, height: 1.4),
                    ),
                  ),
                ],
              ),
            )).toList(),
          ],
        ],
      ),
    );
  }

  Widget _buildFinding(dynamic finding) {
    final map = finding is Map<String, dynamic> ? finding : <String, dynamic>{};
    final severity = map['severity'] as String? ?? 'info';
    final message = map['message'] as String? ?? '';

    IconData icon;
    Color color;
    switch (severity) {
      case 'high':
        icon = Icons.error;
        color = CwColors.danger;
        break;
      case 'medium':
        icon = Icons.warning_amber_rounded;
        color = const Color(0xFFE5A100);
        break;
      default:
        icon = Icons.check_circle_outline;
        color = CwColors.success;
    }

    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 14, color: color),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              message,
              style: const TextStyle(fontSize: 12, color: CwColors.ink2, height: 1.4),
            ),
          ),
        ],
      ),
    );
  }

  String _riskLabel(String level) {
    switch (level) {
      case 'low':
        return '安全';
      case 'medium':
        return '中等风险';
      case 'high':
        return '高风险';
      default:
        return '未知';
    }
  }
}
