use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment, ObjectKind,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType,
    ServiceEntity, ServiceInstance, Subject, TenantId,
};
use uuid::Uuid;

use crate::memory::IngestJobEntry;
use crate::{
    CatalogStore, GovernanceStore, IngestStore, Page, RuntimeStore, StorageResult, not_configured,
};

use super::super::{PostgresExecutor, PostgresTopologyStore, row_decode::*, sql};

impl<E> GovernanceStore for PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()> {
        self.executor.exec(
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
        )?;
        Ok(())
    }

    fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>> {
        let rows = self.executor.query_rows(
            "SELECT assignment_id, tenant_id, subject_id, target_kind, target_id, role, source, valid_from, valid_to, created_at, updated_at FROM responsibility_assignment WHERE assignment_id = $1",
            &[assignment_id.to_string()],
        )?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_responsibility_assignment(&row))
            .transpose()?)
    }

    fn list_responsibility_assignments_for_target(
        &self,
        target_kind: ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>> {
        let rows = self.executor.query_rows(
            sql::LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET,
            &[
                format!("{:?}", target_kind),
                target_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_responsibility_assignment(&row))
            .collect()
    }
}
