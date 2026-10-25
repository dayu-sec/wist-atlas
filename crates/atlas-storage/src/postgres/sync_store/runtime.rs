use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, InstanceEndpoint, NetworkDomain, NetworkSegment,
    ObjectKind, ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType,
    ServiceEntity, ServiceInstance, Subject, TenantId,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::memory::IngestJobEntry;
use crate::{
    CatalogStore, GovernanceStore, IngestStore, Page, RuntimeStore, StorageResult, not_configured,
};

use super::super::{PostgresExecutor, PostgresTopologyStore, row_decode::*, sql};

impl<E> RuntimeStore for PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    fn insert_host_runtime_state(
        &self,
        state: &atlas_domain::HostRuntimeState,
    ) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_HOST_RUNTIME_STATE,
            &[
                state.host_id.to_string(),
                state.observed_at.0.to_rfc3339(),
                state.boot_id.clone().unwrap_or_default(),
                state
                    .uptime_seconds
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state.loadavg_1m.map(|v| v.to_string()).unwrap_or_default(),
                state.loadavg_5m.map(|v| v.to_string()).unwrap_or_default(),
                state.loadavg_15m.map(|v| v.to_string()).unwrap_or_default(),
                state
                    .cpu_usage_pct
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state
                    .memory_used_bytes
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state
                    .memory_available_bytes
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state
                    .process_count
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state
                    .container_count
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                format!("{:?}", state.agent_health),
            ],
        )?;
        Ok(())
    }

    fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::HostRuntimeState>> {
        let rows = self.executor.query_rows(
            sql::LIST_HOST_RUNTIME_STATES,
            &[
                host_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_host_runtime_state(&row))
            .collect()
    }

    fn upsert_process_runtime_state(
        &self,
        state: &atlas_domain::ProcessRuntimeState,
    ) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_PROCESS_RUNTIME_STATE,
            &[
                state.process_id.to_string(),
                state.tenant_id.0.to_string(),
                state.host_id.to_string(),
                state
                    .container_id
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state.external_ref.clone().unwrap_or_default(),
                state.pid.to_string(),
                state.executable.clone(),
                state.command_line.clone().unwrap_or_default(),
                state.process_state.clone().unwrap_or_default(),
                state
                    .memory_rss_kib
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                state.started_at.to_rfc3339(),
                state.observed_at.0.to_rfc3339(),
                state
                    .stale_at
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default(),
                state.ingest_scope.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::ProcessRuntimeState>> {
        let rows = self.executor.query_rows(
            sql::LIST_PROCESS_RUNTIME_STATES,
            &[
                host_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_process_runtime_state(&row))
            .collect()
    }

    fn list_host_process_counts(&self, page: Page) -> StorageResult<Vec<(Uuid, i64)>> {
        let rows = self.executor.query_rows(
            sql::LIST_HOST_PROCESS_COUNTS,
            &[page.limit.to_string(), page.offset.to_string()],
        )?;
        rows.into_iter()
            .map(|row| decode_host_process_count(&row))
            .collect()
    }

    fn upsert_service_instance(
        &self,
        instance: &atlas_domain::ServiceInstance,
    ) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_SERVICE_INSTANCE,
            &[
                instance.instance_id.to_string(),
                instance.tenant_id.0.to_string(),
                instance.service_id.to_string(),
                instance
                    .workload_id
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                instance.started_at.to_rfc3339(),
                instance
                    .ended_at
                    .map(|v| v.to_rfc3339())
                    .unwrap_or_default(),
                instance.last_seen_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_service_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::ServiceInstance>> {
        let rows = self
            .executor
            .query_rows(sql::GET_SERVICE_INSTANCE, &[instance_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_service_instance(&row))
            .transpose()?)
    }

    fn list_service_instances(
        &self,
        service_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::ServiceInstance>> {
        let rows = self.executor.query_rows(
            sql::LIST_SERVICE_INSTANCES,
            &[
                service_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_service_instance(&row))
            .collect()
    }

    fn upsert_instance_endpoint(&self, endpoint: &InstanceEndpoint) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_INSTANCE_ENDPOINT,
            &[
                endpoint.endpoint_id.to_string(),
                endpoint.tenant_id.0.to_string(),
                endpoint.instance_id.to_string(),
                endpoint.address.clone(),
                endpoint.port.to_string(),
                endpoint.protocol.clone(),
                endpoint.source.clone(),
                endpoint.valid_from.to_rfc3339(),
                endpoint
                    .valid_to
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default(),
                endpoint.created_at.to_rfc3339(),
                endpoint.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn list_instance_endpoints(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<InstanceEndpoint>> {
        let rows = self.executor.query_rows(
            sql::LIST_INSTANCE_ENDPOINTS,
            &[
                instance_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_instance_endpoint(&row))
            .collect()
    }

    fn close_instance_endpoint(&self, endpoint_id: Uuid, at: DateTime<Utc>) -> StorageResult<()> {
        self.executor.exec(
            sql::CLOSE_INSTANCE_ENDPOINT,
            &[endpoint_id.to_string(), at.to_rfc3339()],
        )?;
        Ok(())
    }

    fn list_instance_ids_for_host(&self, host_id: Uuid) -> StorageResult<Vec<Uuid>> {
        let rows = self
            .executor
            .query_rows(sql::LIST_INSTANCE_IDS_FOR_HOST, &[host_id.to_string()])?;
        rows.into_iter().map(|row| parse_uuid(&row[0])).collect()
    }

    fn upsert_runtime_binding(&self, binding: &atlas_domain::RuntimeBinding) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_RUNTIME_BINDING,
            &[
                binding.binding_id.to_string(),
                binding.instance_id.to_string(),
                format!("{:?}", binding.object_type),
                binding.object_id.to_string(),
                format!("{:?}", binding.scope),
                format!("{:?}", binding.confidence),
                binding.source.clone(),
                binding.validity.valid_from.to_rfc3339(),
                binding
                    .validity
                    .valid_to
                    .map(|v| v.to_rfc3339())
                    .unwrap_or_default(),
                binding.created_at.to_rfc3339(),
                binding.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_runtime_binding(
        &self,
        binding_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::RuntimeBinding>> {
        let rows = self
            .executor
            .query_rows(sql::GET_RUNTIME_BINDING, &[binding_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_runtime_binding(&row))
            .transpose()?)
    }

    fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::RuntimeBinding>> {
        let rows = self.executor.query_rows(
            sql::LIST_RUNTIME_BINDINGS_FOR_INSTANCE,
            &[
                instance_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_runtime_binding(&row))
            .collect()
    }

    fn list_runtime_bindings_for_object(
        &self,
        object_type: RuntimeObjectType,
        object_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::RuntimeBinding>> {
        let rows = self.executor.query_rows(
            sql::LIST_RUNTIME_BINDINGS_FOR_OBJECT,
            &[
                format!("{:?}", object_type),
                object_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_runtime_binding(&row))
            .collect()
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
        self.executor.exec(
            sql::UPSERT_HOST_NET_ASSOC,
            &[
                assoc.assoc_id.to_string(),
                assoc.tenant_id.0.to_string(),
                assoc.host_id.to_string(),
                assoc.network_segment_id.to_string(),
                assoc.ip_addr.clone(),
                assoc.iface_name.clone().unwrap_or_default(),
                assoc.validity.valid_from.to_rfc3339(),
                assoc
                    .validity
                    .valid_to
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default(),
                assoc.created_at.to_rfc3339(),
                assoc.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn list_host_net_assocs(&self, host_id: Uuid, page: Page) -> StorageResult<Vec<HostNetAssoc>> {
        let rows = self.executor.query_rows(
            sql::LIST_HOST_NET_ASSOCS,
            &[
                host_id.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_host_net_assoc(&row))
            .collect()
    }
}
