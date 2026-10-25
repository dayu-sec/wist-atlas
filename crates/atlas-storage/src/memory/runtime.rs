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

impl RuntimeStore for InMemoryTopologyStore {
    fn insert_host_runtime_state(
        &self,
        state: &atlas_domain::HostRuntimeState,
    ) -> StorageResult<()> {
        self.with_state(|store| {
            if let Some(existing) = store
                .host_runtime_states
                .iter_mut()
                .find(|item| item.host_id == state.host_id && item.observed_at == state.observed_at)
            {
                *existing = state.clone();
            } else {
                store.host_runtime_states.push(state.clone());
            }
        })
    }

    fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::HostRuntimeState>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .host_runtime_states
                .iter()
                .filter(|item| item.host_id == host_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_process_runtime_state(
        &self,
        state: &atlas_domain::ProcessRuntimeState,
    ) -> StorageResult<()> {
        self.with_state(|store| {
            if let Some(existing) = store
                .process_runtime_states
                .iter_mut()
                .find(|item| item.process_id == state.process_id)
            {
                *existing = state.clone();
            } else {
                store.process_runtime_states.push(state.clone());
            }
        })
    }

    fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::ProcessRuntimeState>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .process_runtime_states
                .iter()
                .filter(|item| item.host_id == host_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_service_instance(
        &self,
        instance: &atlas_domain::ServiceInstance,
    ) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .service_instances
                .iter_mut()
                .find(|item| item.instance_id == instance.instance_id)
            {
                *existing = instance.clone();
            } else {
                state.service_instances.push(instance.clone());
            }
        })
    }

    fn get_service_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::ServiceInstance>> {
        self.with_state(|state| {
            state
                .service_instances
                .iter()
                .find(|item| item.instance_id == instance_id)
                .cloned()
        })
    }

    fn list_service_instances(
        &self,
        service_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::ServiceInstance>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .service_instances
                .iter()
                .filter(|item| item.service_id == service_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_runtime_binding(&self, binding: &atlas_domain::RuntimeBinding) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .runtime_bindings
                .iter_mut()
                .find(|item| item.binding_id == binding.binding_id)
            {
                *existing = binding.clone();
            } else {
                state.runtime_bindings.push(binding.clone());
            }
        })
    }

    fn get_runtime_binding(
        &self,
        binding_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::RuntimeBinding>> {
        self.with_state(|state| {
            state
                .runtime_bindings
                .iter()
                .find(|item| item.binding_id == binding_id)
                .cloned()
        })
    }

    fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::RuntimeBinding>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .runtime_bindings
                .iter()
                .filter(|item| item.instance_id == instance_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn list_runtime_bindings_for_object(
        &self,
        object_type: atlas_domain::RuntimeObjectType,
        object_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::RuntimeBinding>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .runtime_bindings
                .iter()
                .filter(|item| item.object_type == object_type && item.object_id == object_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_workload_pod_membership(
        &self,
        _membership: &atlas_domain::WorkloadPodMembership,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    fn upsert_pod_placement(&self, _placement: &atlas_domain::PodPlacement) -> StorageResult<()> {
        Err(not_configured())
    }

    fn upsert_host_net_assoc(&self, assoc: &HostNetAssoc) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .host_net_assocs
                .iter_mut()
                .find(|item| item.assoc_id == assoc.assoc_id)
            {
                *existing = assoc.clone();
            } else {
                state.host_net_assocs.push(assoc.clone());
            }
        })
    }

    fn list_host_net_assocs(&self, host_id: Uuid, page: Page) -> StorageResult<Vec<HostNetAssoc>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .host_net_assocs
                .iter()
                .filter(|item| item.host_id == host_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }
}
