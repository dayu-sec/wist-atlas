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

impl AsyncCatalogStore for PostgresTopologyStore<LivePostgresExecutor> {
    async fn upsert_business(&self, _business: &atlas_domain::BusinessDomain) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_business(
        &self,
        _business_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::BusinessDomain>> {
        Ok(None)
    }

    async fn list_businesses(
        &self,
        _tenant_id: TenantId,
        _page: Page,
    ) -> StorageResult<Vec<atlas_domain::BusinessDomain>> {
        Ok(Vec::new())
    }

    async fn upsert_system(&self, _system: &atlas_domain::SystemBoundary) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_system(
        &self,
        _system_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::SystemBoundary>> {
        Ok(None)
    }

    async fn upsert_subsystem(&self, _subsystem: &atlas_domain::Subsystem) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_subsystem(
        &self,
        _subsystem_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::Subsystem>> {
        Ok(None)
    }

    async fn upsert_service(&self, service: &ServiceEntity) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_SERVICE,
                &[
                    service.service_id.to_string(),
                    service.tenant_id.0.to_string(),
                    service
                        .business_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    service
                        .system_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    service
                        .subsystem_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    service.name.clone(),
                    service.namespace.clone().unwrap_or_default(),
                    format!("{:?}", service.service_type),
                    format!("{:?}", service.boundary),
                    service.provider.clone().unwrap_or_default(),
                    service.external_ref.clone().unwrap_or_default(),
                    service.created_at.to_rfc3339(),
                    service.updated_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_service(&self, service_id: Uuid) -> StorageResult<Option<ServiceEntity>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_SERVICE, &[service_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_service(&row))
            .transpose()?)
    }

    async fn upsert_host(&self, host: &HostInventory) -> StorageResult<()> {
        let sql = if host.machine_id.is_some() {
            sql::UPSERT_HOST
        } else {
            sql::UPSERT_HOST_WITHOUT_MACHINE_ID
        };
        self.executor
            .exec_async(
                sql,
                &[
                    host.host_id.to_string(),
                    host.tenant_id.0.to_string(),
                    host.environment_id
                        .map(|id| id.0.to_string())
                        .unwrap_or_default(),
                    host.host_name.clone(),
                    host.machine_id.clone().unwrap_or_default(),
                    host.os_name.clone().unwrap_or_default(),
                    host.os_version.clone().unwrap_or_default(),
                    host.created_at.to_rfc3339(),
                    host.last_inventory_at.to_rfc3339(),
                    host.arch.clone().unwrap_or_default(),
                    host.stale_at
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_default(),
                    host.ingest_scope.clone().unwrap_or_default(),
                    host.miss_count.to_string(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_HOST, &[host_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_host(&row))
            .transpose()?)
    }

    async fn list_hosts(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<HostInventory>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_HOSTS,
                &[
                    tenant_id.0.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter().map(|row| decode_host(&row)).collect()
    }

    async fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_ALL_HOSTS,
                &[page.limit.to_string(), page.offset.to_string()],
            )
            .await?;
        rows.into_iter().map(|row| decode_host(&row)).collect()
    }

    async fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_NETWORK_DOMAIN,
                &[
                    domain.network_domain_id.to_string(),
                    domain.tenant_id.0.to_string(),
                    domain
                        .environment_id
                        .map(|id| id.0.to_string())
                        .unwrap_or_default(),
                    domain.name.clone(),
                    format!("{:?}", domain.kind),
                    domain.description.clone().unwrap_or_default(),
                    domain.created_at.to_rfc3339(),
                    domain.updated_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_NETWORK_SEGMENT, &[network_segment_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_network_segment(&row))
            .transpose()?)
    }

    async fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_NETWORK_SEGMENT,
                &[
                    segment.network_segment_id.to_string(),
                    segment.tenant_id.0.to_string(),
                    segment
                        .network_domain_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    segment
                        .environment_id
                        .map(|id| id.0.to_string())
                        .unwrap_or_default(),
                    segment.name.clone(),
                    segment.cidr.clone().unwrap_or_default(),
                    segment.gateway_ip.clone().unwrap_or_default(),
                    format!("{:?}", segment.address_family),
                    segment.created_at.to_rfc3339(),
                    segment.updated_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_NETWORK_SEGMENTS,
                &[
                    tenant_id.0.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_network_segment(&row))
            .collect()
    }

    async fn list_services(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<ServiceEntity>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_SERVICES,
                &[
                    tenant_id.0.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter().map(|row| decode_service(&row)).collect()
    }

    async fn upsert_subject(&self, subject: &Subject) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_SUBJECT,
                &[
                    subject.subject_id.to_string(),
                    subject.tenant_id.0.to_string(),
                    format!("{:?}", subject.subject_type),
                    subject.display_name.clone(),
                    subject.external_ref.clone().unwrap_or_default(),
                    subject.email.clone().unwrap_or_default(),
                    subject.is_active.to_string(),
                    subject.created_at.to_rfc3339(),
                    subject.updated_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_SUBJECT, &[subject_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_subject(&row))
            .transpose()?)
    }

    async fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>> {
        let rows = self
            .executor
            .query_rows_async(
                "SELECT subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active, created_at, updated_at FROM subject WHERE tenant_id = $1 ORDER BY display_name ASC, subject_id ASC LIMIT $2 OFFSET $3",
                &[
                    tenant_id.0.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter().map(|row| decode_subject(&row)).collect()
    }

    async fn upsert_cluster(&self, _cluster: &atlas_domain::ClusterInventory) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_cluster(
        &self,
        _cluster_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::ClusterInventory>> {
        Ok(None)
    }

    async fn upsert_namespace(
        &self,
        _namespace: &atlas_domain::NamespaceInventory,
    ) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_namespace(
        &self,
        _namespace_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::NamespaceInventory>> {
        Ok(None)
    }

    async fn upsert_workload(&self, _workload: &atlas_domain::WorkloadEntity) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_workload(
        &self,
        _workload_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::WorkloadEntity>> {
        Ok(None)
    }

    async fn upsert_pod(&self, _pod: &atlas_domain::PodInventory) -> StorageResult<()> {
        Err(not_configured())
    }

    async fn get_pod(&self, _pod_id: Uuid) -> StorageResult<Option<atlas_domain::PodInventory>> {
        Ok(None)
    }

    async fn get_network_domain(
        &self,
        network_domain_id: Uuid,
    ) -> StorageResult<Option<NetworkDomain>> {
        let rows = self
            .executor
            .query_rows_async(
                "SELECT network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at FROM network_domain WHERE network_domain_id = $1",
                &[network_domain_id.to_string()],
            )
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_network_domain(&row))
            .transpose()?)
    }
}
