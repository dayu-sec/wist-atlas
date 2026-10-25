use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment, ObjectKind,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType,
    ServiceEntity, ServiceInstance, Subject, TenantId,
};
use uuid::Uuid;

use crate::memory::IngestJobEntry;
use crate::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncIngestStore, AsyncRuntimeStore, Page,
    StorageResult, not_configured,
};

use super::super::{LivePostgresExecutor, PostgresTopologyStore, row_decode::*, sql};

impl AsyncGovernanceStore for PostgresTopologyStore<LivePostgresExecutor> {
    async fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_RESPONSIBILITY_ASSIGNMENT,
                &[
                    assignment.assignment_id.to_string(),
                    assignment.tenant_id.0.to_string(),
                    assignment.subject_id.to_string(),
                    format!("{:?}", assignment.target_kind),
                    assignment.target_id.to_string(),
                    format!("{:?}", assignment.role),
                    assignment.source.clone(),
                    assignment.validity.valid_from.to_rfc3339(),
                    assignment
                        .validity
                        .valid_to
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_default(),
                    assignment.created_at.to_rfc3339(),
                    assignment.updated_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn list_responsibility_assignments_for_target(
        &self,
        target_kind: ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET,
                &[
                    format!("{:?}", target_kind),
                    target_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_responsibility_assignment(&row))
            .collect()
    }

    async fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>> {
        let rows = self
            .executor
            .query_rows_async(
                "SELECT assignment_id, tenant_id, subject_id, target_kind, target_id, role, source, valid_from, valid_to, created_at, updated_at FROM responsibility_assignment WHERE assignment_id = $1",
                &[assignment_id.to_string()],
            )
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_responsibility_assignment(&row))
            .transpose()?)
    }
}
