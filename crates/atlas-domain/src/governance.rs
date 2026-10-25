use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ObjectKind, TenantId, ValidityWindow};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResponsibilityRole {
    Owner,
    Maintainer,
    Oncall,
    Security,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponsibilityAssignment {
    pub assignment_id: Uuid,
    pub tenant_id: TenantId,
    pub subject_id: Uuid,
    pub target_kind: ObjectKind,
    pub target_id: Uuid,
    pub role: ResponsibilityRole,
    pub source: String,
    pub validity: ValidityWindow,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
