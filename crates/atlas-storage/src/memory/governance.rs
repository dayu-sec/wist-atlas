use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, ServiceEntity, ServiceInstance,
    Subject, TenantId,
};
use uuid::Uuid;

use crate::{
    CatalogStore, GovernanceStore, IngestStore, Page, RuntimeStore, StorageResult, not_configured,
};

use super::{InMemoryTopologyStore, IngestJobEntry};

impl GovernanceStore for InMemoryTopologyStore {
    fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .responsibility_assignments
                .iter_mut()
                .find(|item| item.assignment_id == assignment.assignment_id)
            {
                *existing = assignment.clone();
            } else {
                state.responsibility_assignments.push(assignment.clone());
            }
        })
    }

    fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>> {
        self.with_state(|state| {
            state
                .responsibility_assignments
                .iter()
                .find(|item| item.assignment_id == assignment_id)
                .cloned()
        })
    }

    fn list_responsibility_assignments_for_target(
        &self,
        target_kind: atlas_domain::ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .responsibility_assignments
                .iter()
                .filter(|item| item.target_kind == target_kind && item.target_id == target_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }
}
