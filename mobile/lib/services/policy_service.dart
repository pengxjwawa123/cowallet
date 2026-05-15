import '../api/policy_api.dart';
import '../network/result.dart';

/// Policy evaluation decision from the backend.
enum PolicyDecision {
  /// Transaction is allowed to proceed.
  allow,

  /// Transaction requires additional co-signer approval.
  requireApproval,

  /// Transaction is denied by policy.
  deny,
}

/// Result of a policy evaluation against a transaction.
class PolicyCheckResult {
  final PolicyDecision decision;

  /// Human-readable reason for the decision (e.g. "Exceeds daily limit of $500").
  final String? reason;

  /// The name of the triggered policy rule, if any.
  final String? policyName;

  /// Additional details from the backend (e.g. threshold, current spend).
  final Map<String, dynamic>? details;

  const PolicyCheckResult({
    required this.decision,
    this.reason,
    this.policyName,
    this.details,
  });

  /// A pass-through result when no policy applies.
  static const PolicyCheckResult allowed = PolicyCheckResult(
    decision: PolicyDecision.allow,
  );
}

/// Service that evaluates transactions against the policy engine before signing.
class PolicyService {
  /// Check whether a transaction is allowed by policy rules.
  ///
  /// [from] sender address
  /// [to] recipient address
  /// [value] amount in wei (as string)
  /// [token] token symbol (e.g. "ETH", "USDC")
  /// [chainId] target chain ID
  /// [amountUsd] optional USD-equivalent amount for limit checks
  Future<PolicyCheckResult> checkTransaction({
    required String from,
    required String to,
    required String value,
    required String token,
    required int chainId,
    double? amountUsd,
  }) async {
    try {
      final txData = <String, dynamic>{
        'from': from,
        'to': to,
        'value': value,
        'token': token,
        'chain_id': chainId,
      };
      if (amountUsd != null) {
        txData['amount_usd'] = amountUsd;
      }

      final Result<Map<String, dynamic>> result =
          await PolicyApi.evaluateTransaction(txData: txData);

      if (!result.isSuccess || result.data == null) {
        // If policy service is unreachable, default to allow (fail-open)
        // to avoid blocking users. The backend will enforce policies server-side
        // during tx submission anyway.
        return PolicyCheckResult.allowed;
      }

      final data = result.data!;
      final String action = data['action'] ?? 'allow';
      final String? message = data['message'] as String?;
      final String? ruleName = data['policy_name'] as String?;
      final Map<String, dynamic>? details =
          data['details'] is Map<String, dynamic>
              ? data['details'] as Map<String, dynamic>
              : null;

      switch (action) {
        case 'deny':
          return PolicyCheckResult(
            decision: PolicyDecision.deny,
            reason: message,
            policyName: ruleName,
            details: details,
          );
        case 'confirm':
        case 'require_approval':
          return PolicyCheckResult(
            decision: PolicyDecision.requireApproval,
            reason: message,
            policyName: ruleName,
            details: details,
          );
        case 'allow':
        default:
          return PolicyCheckResult.allowed;
      }
    } catch (_) {
      // Fail-open: if evaluation throws, allow the transaction.
      // Server-side enforcement is the final gate.
      return PolicyCheckResult.allowed;
    }
  }
}
