use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, InstanceEndpoint, NetworkDomain, NetworkSegment,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, ServiceEntity, ServiceInstance,
    Subject, TenantId,
};
use chrono::{DateTime, Utc};
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

    fn list_host_process_counts(&self, page: Page) -> StorageResult<Vec<(Uuid, i64)>> {
        self.with_state(|state| {
            let mut counts: std::collections::BTreeMap<Uuid, i64> =
                std::collections::BTreeMap::new();
            for item in state
                .process_runtime_states
                .iter()
                .filter(|item| item.stale_at.is_none())
            {
                *counts.entry(item.host_id).or_insert(0) += 1;
            }
            // Uuid 的 BTreeMap 序与 SQL 的 ORDER BY host_id 一致。
            counts
                .into_iter()
                .skip(page.offset as usize)
                .take(page.limit as usize)
                .collect()
        })
    }

    fn upsert_container_runtime(
        &self,
        container: &atlas_domain::ContainerRuntime,
    ) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .container_runtimes
                .iter_mut()
                .find(|item| item.container_id == container.container_id)
            {
                *existing = container.clone();
            } else {
                state.container_runtimes.push(container.clone());
            }
        })
    }

    fn list_container_runtimes_for_host(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::ContainerRuntime>> {
        self.with_state(|state| {
            let mut rows: Vec<atlas_domain::ContainerRuntime> = state
                .container_runtimes
                .iter()
                .filter(|item| item.host_id == host_id && item.stale_at.is_none())
                .cloned()
                .collect();
            // 与 SQL 的 `ORDER BY container_name, container_id` 对齐。
            rows.sort_by(|left, right| {
                left.container_name
                    .cmp(&right.container_name)
                    .then(left.container_id.cmp(&right.container_id))
            });
            rows.into_iter()
                .skip(page.offset as usize)
                .take(page.limit as usize)
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

    fn upsert_instance_endpoint(&self, endpoint: &InstanceEndpoint) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .instance_endpoints
                .iter_mut()
                .find(|item| item.endpoint_id == endpoint.endpoint_id)
            {
                // 与 SQL 里的 CASE 对齐：窗口未关就保留最早的起点，
                // 关过又重开则重新计（否则重开后的 valid_from 会跑到上一段窗口）。
                let valid_from = if existing.valid_to.is_some() {
                    endpoint.valid_from
                } else {
                    existing.valid_from.min(endpoint.valid_from)
                };
                *existing = InstanceEndpoint {
                    valid_from,
                    ..endpoint.clone()
                };
            } else {
                state.instance_endpoints.push(endpoint.clone());
            }
        })
    }

    fn list_instance_endpoints(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<InstanceEndpoint>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            let mut rows: Vec<InstanceEndpoint> = state
                .instance_endpoints
                .iter()
                .filter(|item| item.instance_id == instance_id && item.valid_to.is_none())
                .cloned()
                .collect();
            // 与 SQL 的 ORDER BY protocol, address, port 对齐：顺序不同会让测试替身
            // 掩盖掉真实数据库的分页差异。
            rows.sort_by(|left, right| {
                (&left.protocol, &left.address, left.port).cmp(&(
                    &right.protocol,
                    &right.address,
                    right.port,
                ))
            });
            rows.into_iter()
                .skip(start)
                .take(page.limit as usize)
                .collect()
        })
    }

    fn close_instance_endpoint(&self, endpoint_id: Uuid, at: DateTime<Utc>) -> StorageResult<()> {
        self.with_state(|state| {
            for item in state.instance_endpoints.iter_mut() {
                if item.endpoint_id == endpoint_id && item.valid_to.is_none() {
                    item.valid_to = Some(at);
                    item.updated_at = at;
                }
            }
        })
    }

    fn list_instance_ids_for_host(&self, host_id: Uuid) -> StorageResult<Vec<Uuid>> {
        self.with_state(|state| {
            let live_process_ids: std::collections::BTreeSet<Uuid> = state
                .process_runtime_states
                .iter()
                .filter(|item| item.host_id == host_id && item.stale_at.is_none())
                .map(|item| item.process_id)
                .collect();
            let mut instance_ids: Vec<Uuid> = state
                .runtime_bindings
                .iter()
                .filter(|item| {
                    item.object_type == atlas_domain::RuntimeObjectType::Process
                        && live_process_ids.contains(&item.object_id)
                })
                .map(|item| item.instance_id)
                .collect();
            // 与 SQL 的 DISTINCT 对齐：一个实例可能有多个进程绑定。
            instance_ids.sort();
            instance_ids.dedup();
            instance_ids
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
