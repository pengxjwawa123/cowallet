use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An approval request for high-risk transactions requiring M-of-N sign-off.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub transaction_id: String,
    pub required_approvals: u32,
    pub approvers: Vec<String>,
    pub received_approvals: Vec<Approval>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: ApprovalStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub approver_id: String,
    pub approved: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

impl ApprovalRequest {
    pub fn new(
        transaction_id: String,
        approvers: Vec<String>,
        threshold: u32,
        ttl_hours: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            transaction_id,
            required_approvals: threshold,
            approvers,
            received_approvals: Vec::new(),
            created_at: now,
            expires_at: now + chrono::Duration::hours(ttl_hours as i64),
            status: ApprovalStatus::Pending,
        }
    }

    /// Record an approval or rejection from an approver.
    pub fn add_approval(&mut self, approver_id: &str, approved: bool) -> bool {
        if !self.approvers.contains(&approver_id.to_string()) {
            return false;
        }

        if self
            .received_approvals
            .iter()
            .any(|a| a.approver_id == approver_id)
        {
            return false;
        }

        self.received_approvals.push(Approval {
            approver_id: approver_id.to_string(),
            approved,
            timestamp: Utc::now(),
        });

        self.update_status();
        true
    }

    fn update_status(&mut self) {
        if Utc::now() > self.expires_at {
            self.status = ApprovalStatus::Expired;
            return;
        }

        let approved_count = self
            .received_approvals
            .iter()
            .filter(|a| a.approved)
            .count() as u32;
        let rejected_count = self
            .received_approvals
            .iter()
            .filter(|a| !a.approved)
            .count() as u32;
        let remaining = self.approvers.len() as u32 - (approved_count + rejected_count);

        if approved_count >= self.required_approvals {
            self.status = ApprovalStatus::Approved;
        } else if approved_count + remaining < self.required_approvals {
            self.status = ApprovalStatus::Rejected;
        }
    }

    pub fn is_approved(&self) -> bool {
        self.status == ApprovalStatus::Approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_2_of_3_approval() {
        let approvers = vec!["alice".into(), "bob".into(), "carol".into()];
        let mut req = ApprovalRequest::new("tx-001".into(), approvers, 2, 24);

        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("alice", true);
        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("bob", true);
        assert_eq!(req.status, ApprovalStatus::Approved);
    }

    #[test]
    fn test_rejection_when_impossible() {
        let approvers = vec!["alice".into(), "bob".into(), "carol".into()];
        let mut req = ApprovalRequest::new("tx-002".into(), approvers, 2, 24);

        req.add_approval("alice", false);
        req.add_approval("bob", false);
        assert_eq!(req.status, ApprovalStatus::Rejected);
    }

    #[test]
    fn test_pending_approval_creation() {
        let approvers = vec!["alice".into(), "bob".into()];
        let req = ApprovalRequest::new("tx-123".into(), approvers.clone(), 1, 24);

        assert_eq!(req.transaction_id, "tx-123");
        assert_eq!(req.required_approvals, 1);
        assert_eq!(req.approvers, approvers);
        assert_eq!(req.received_approvals.len(), 0);
        assert_eq!(req.status, ApprovalStatus::Pending);
        assert!(!req.is_approved());
    }

    #[test]
    fn test_1_of_1_approval() {
        let approvers = vec!["alice".into()];
        let mut req = ApprovalRequest::new("tx-003".into(), approvers, 1, 24);

        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("alice", true);
        assert_eq!(req.status, ApprovalStatus::Approved);
        assert!(req.is_approved());
    }

    #[test]
    fn test_3_of_5_approval() {
        let approvers = vec![
            "alice".into(),
            "bob".into(),
            "carol".into(),
            "dave".into(),
            "eve".into(),
        ];
        let mut req = ApprovalRequest::new("tx-004".into(), approvers, 3, 24);

        req.add_approval("alice", true);
        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("bob", true);
        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("carol", true);
        assert_eq!(req.status, ApprovalStatus::Approved);
    }

    #[test]
    fn test_cannot_approve_twice() {
        let approvers = vec!["alice".into(), "bob".into()];
        let mut req = ApprovalRequest::new("tx-005".into(), approvers, 2, 24);

        let result1 = req.add_approval("alice", true);
        assert!(result1);
        assert_eq!(req.received_approvals.len(), 1);

        // Try to approve again
        let result2 = req.add_approval("alice", true);
        assert!(!result2);
        assert_eq!(req.received_approvals.len(), 1);
    }

    #[test]
    fn test_non_approver_cannot_approve() {
        let approvers = vec!["alice".into(), "bob".into()];
        let mut req = ApprovalRequest::new("tx-006".into(), approvers, 2, 24);

        let result = req.add_approval("mallory", true);
        assert!(!result);
        assert_eq!(req.received_approvals.len(), 0);
    }

    #[test]
    fn test_mixed_approvals_and_rejections() {
        let approvers = vec!["alice".into(), "bob".into(), "carol".into()];
        let mut req = ApprovalRequest::new("tx-007".into(), approvers, 2, 24);

        req.add_approval("alice", true);
        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("bob", false);
        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("carol", true);
        assert_eq!(req.status, ApprovalStatus::Approved);
    }

    #[test]
    fn test_rejection_when_not_enough_approvers_left() {
        let approvers = vec![
            "alice".into(),
            "bob".into(),
            "carol".into(),
            "dave".into(),
        ];
        let mut req = ApprovalRequest::new("tx-008".into(), approvers, 3, 24);

        req.add_approval("alice", false);
        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("bob", false);
        // Only 2 approvers left, but need 3 approvals
        assert_eq!(req.status, ApprovalStatus::Rejected);
    }

    #[test]
    fn test_approval_timestamp_recorded() {
        let approvers = vec!["alice".into()];
        let mut req = ApprovalRequest::new("tx-009".into(), approvers, 1, 24);

        let before = Utc::now();
        req.add_approval("alice", true);
        let after = Utc::now();

        assert_eq!(req.received_approvals.len(), 1);
        let approval = &req.received_approvals[0];
        assert_eq!(approval.approver_id, "alice");
        assert!(approval.approved);
        assert!(approval.timestamp >= before && approval.timestamp <= after);
    }

    #[test]
    fn test_approval_expiration_tracking() {
        let approvers = vec!["alice".into()];
        let req = ApprovalRequest::new("tx-010".into(), approvers, 1, 48);

        let expected_expiry = req.created_at + chrono::Duration::hours(48);
        assert_eq!(req.expires_at, expected_expiry);
    }

    #[test]
    fn test_approval_request_fields() {
        let approvers = vec!["alice".into(), "bob".into()];
        let tx_id = "tx-special-001".to_string();
        let req = ApprovalRequest::new(tx_id.clone(), approvers.clone(), 2, 12);

        assert_eq!(req.transaction_id, tx_id);
        assert_eq!(req.required_approvals, 2);
        assert_eq!(req.approvers.len(), 2);
        assert!(req.approvers.contains(&"alice".to_string()));
        assert!(req.approvers.contains(&"bob".to_string()));
    }

    #[test]
    fn test_rejection_flag_recorded() {
        let approvers = vec!["alice".into(), "bob".into()];
        let mut req = ApprovalRequest::new("tx-011".into(), approvers, 2, 24);

        req.add_approval("alice", false);

        assert_eq!(req.received_approvals.len(), 1);
        let rejection = &req.received_approvals[0];
        assert_eq!(rejection.approver_id, "alice");
        assert!(!rejection.approved);
    }

    #[test]
    fn test_unanimous_approval_required() {
        let approvers = vec!["alice".into(), "bob".into(), "carol".into()];
        let mut req = ApprovalRequest::new("tx-012".into(), approvers, 3, 24);

        req.add_approval("alice", true);
        req.add_approval("bob", true);
        assert_eq!(req.status, ApprovalStatus::Pending);

        req.add_approval("carol", true);
        assert_eq!(req.status, ApprovalStatus::Approved);
    }
}
