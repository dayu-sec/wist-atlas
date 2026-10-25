use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ObjectKind, TenantId};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExternalSystemType {
    Cmdb,
    Ldap,
    Iam,
    Hr,
    Oncall,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExternalObjectType {
    Host,
    HostGroup,
    User,
    Team,
    Rotation,
    Service,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExternalLinkStatus {
    Active,
    Stale,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalIdentityLink {
    pub link_id: Uuid,
    pub tenant_id: TenantId,
    pub system_type: ExternalSystemType,
    pub object_type: ExternalObjectType,
    pub external_id: String,
    pub external_key: Option<String>,
    pub internal_kind: ObjectKind,
    pub internal_id: Uuid,
    pub status: ExternalLinkStatus,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub last_synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalSyncCursor {
    pub cursor_id: Uuid,
    pub tenant_id: TenantId,
    pub system_type: ExternalSystemType,
    pub scope_key: String,
    pub cursor_value: Option<String>,
    pub full_sync_token: Option<String>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}
