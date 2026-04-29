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
}
