use atlas_domain::{
    ElasticIp, HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment,
    ObjectKind, ProcessRuntimeState, ResponsibilityAssignment, RetirementOwner, RuntimeBinding,
    RuntimeObjectType, ServiceEntity, ServiceInstance, Subject, TenantId,
};
use uuid::Uuid;

use crate::memory::IngestJobEntry;
use crate::{
    CatalogStore, GovernanceStore, IngestStore, Page, RuntimeStore, StorageResult, not_configured,
};

use super::super::{PostgresExecutor, PostgresTopologyStore, row_decode::*, sql};

impl<E> CatalogStore for PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    fn upsert_business(&self, _business: &atlas_domain::BusinessDomain) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_business(
        &self,
        _business_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::BusinessDomain>> {
        Ok(None)
    }

    fn list_businesses(
        &self,
        _tenant_id: TenantId,
        _page: Page,
    ) -> StorageResult<Vec<atlas_domain::BusinessDomain>> {
        Ok(Vec::new())
    }

    fn upsert_system(&self, _system: &atlas_domain::SystemBoundary) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_system(&self, _system_id: Uuid) -> StorageResult<Option<atlas_domain::SystemBoundary>> {
        Ok(None)
    }

    fn upsert_subsystem(&self, _subsystem: &atlas_domain::Subsystem) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_subsystem(&self, _subsystem_id: Uuid) -> StorageResult<Option<atlas_domain::Subsystem>> {
        Ok(None)
    }

    fn upsert_service(&self, service: &atlas_domain::ServiceEntity) -> StorageResult<()> {
        self.executor.exec(
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
        )?;
        Ok(())
    }

    fn get_service(&self, service_id: Uuid) -> StorageResult<Option<atlas_domain::ServiceEntity>> {
        let rows = self
            .executor
            .query_rows(sql::GET_SERVICE, &[service_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_service(&row))
            .transpose()?)
    }

    fn list_services(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::ServiceEntity>> {
        let rows = self.executor.query_rows(
            sql::LIST_SERVICES,
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter().map(|row| decode_service(&row)).collect()
    }

    fn upsert_cluster(&self, _cluster: &atlas_domain::ClusterInventory) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_cluster(
        &self,
        _cluster_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::ClusterInventory>> {
        Ok(None)
    }

    fn upsert_namespace(&self, _namespace: &atlas_domain::NamespaceInventory) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_namespace(
        &self,
        _namespace_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::NamespaceInventory>> {
        Ok(None)
    }

    fn upsert_workload(&self, _workload: &atlas_domain::WorkloadEntity) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_workload(
        &self,
        _workload_id: Uuid,
    ) -> StorageResult<Option<atlas_domain::WorkloadEntity>> {
        Ok(None)
    }

    fn upsert_pod(&self, _pod: &atlas_domain::PodInventory) -> StorageResult<()> {
        Err(not_configured())
    }

    fn get_pod(&self, _pod_id: Uuid) -> StorageResult<Option<atlas_domain::PodInventory>> {
        Ok(None)
    }

    fn upsert_host(&self, host: &HostInventory) -> StorageResult<()> {
        // 仲裁列是主键（见 `sql::UPSERT_HOST` 的注释）：带不带 machine_id 走同一条语句。
        let sql = sql::UPSERT_HOST;
        self.executor.exec(
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
                host.cloud_instance_id.clone().unwrap_or_default(),
                host.cloud_account.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>> {
        let rows = self
            .executor
            .query_rows(sql::GET_HOST, &[host_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_host(&row))
            .transpose()?)
    }

    fn list_hosts(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<HostInventory>> {
        let rows = self.executor.query_rows(
            sql::LIST_HOSTS,
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter().map(|row| decode_host(&row)).collect()
    }

    fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>> {
        let rows = self.executor.query_rows(
            sql::LIST_ALL_HOSTS,
            &[page.limit.to_string(), page.offset.to_string()],
        )?;
        rows.into_iter().map(|row| decode_host(&row)).collect()
    }

    fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()> {
        self.executor.exec(
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
                domain.external_ref.clone().unwrap_or_default(),
                domain.cidr.clone().unwrap_or_default(),
                domain.cloud_account.clone().unwrap_or_default(),
                domain
                    .stale_at
                    .map(|at| at.to_rfc3339())
                    .unwrap_or_default(),
                domain.miss_count.to_string(),
                domain.ingest_scope.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    fn get_network_domain(&self, network_domain_id: Uuid) -> StorageResult<Option<NetworkDomain>> {
        let rows = self.executor.query_rows(
            "SELECT network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at, external_ref, cidr, cloud_account, stale_at, miss_count, ingest_scope FROM network_domain WHERE network_domain_id = NULLIF($1, '')::uuid",
            &[network_domain_id.to_string()],
        )?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_network_domain(&row))
            .transpose()?)
    }

    fn list_network_domains(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkDomain>> {
        let rows = self.executor.query_rows(
            sql::LIST_NETWORK_DOMAINS,
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_network_domain(&row))
            .collect()
    }

    fn get_retirement_owner(
        &self,
        tenant_id: TenantId,
        cloud_account: &str,
    ) -> StorageResult<Option<RetirementOwner>> {
        let rows = self.executor.query_rows(
            sql::GET_RETIREMENT_OWNER,
            &[tenant_id.0.to_string(), cloud_account.to_string()],
        )?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_retirement_owner(&row))
            .transpose()?)
    }

    fn upsert_retirement_owner(&self, owner: &RetirementOwner) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_RETIREMENT_OWNER,
            &[
                owner.tenant_id.0.to_string(),
                owner.cloud_account.clone(),
                owner.producer.clone(),
                owner.claimed_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()> {
        self.executor.exec(
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
                segment.external_ref.clone().unwrap_or_default(),
                segment.cloud_account.clone().unwrap_or_default(),
                segment
                    .stale_at
                    .map(|at| at.to_rfc3339())
                    .unwrap_or_default(),
                segment.miss_count.to_string(),
                segment.ingest_scope.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>> {
        let rows = self
            .executor
            .query_rows(sql::GET_NETWORK_SEGMENT, &[network_segment_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_network_segment(&row))
            .transpose()?)
    }

    fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>> {
        let rows = self.executor.query_rows(
            sql::LIST_NETWORK_SEGMENTS,
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_network_segment(&row))
            .collect()
    }

    fn upsert_elastic_ip(&self, ip: &ElasticIp) -> StorageResult<()> {
        let binding = ip.binding.as_ref();
        self.executor.exec(
            sql::UPSERT_ELASTIC_IP,
            &[
                ip.elastic_ip_id.to_string(),
                ip.tenant_id.0.to_string(),
                ip.environment_id
                    .map(|id| id.0.to_string())
                    .unwrap_or_default(),
                ip.external_ref.clone(),
                ip.address.clone(),
                ip.status.as_str().to_string(),
                ip.charge_type.clone().unwrap_or_default(),
                ip.bandwidth_mbps
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                ip.cloud_account.clone().unwrap_or_default(),
                binding
                    .map(|b| b.target_kind.as_str().to_string())
                    .unwrap_or_default(),
                binding
                    .and_then(|b| b.host_id)
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                binding
                    .and_then(|b| b.external_target_ref.clone())
                    .unwrap_or_default(),
                binding
                    .and_then(|b| b.bound_since)
                    .map(|at| at.to_rfc3339())
                    .unwrap_or_default(),
                ip.created_at.to_rfc3339(),
                ip.last_inventory_at.to_rfc3339(),
                ip.stale_at.map(|at| at.to_rfc3339()).unwrap_or_default(),
                ip.miss_count.to_string(),
                ip.ingest_scope.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    fn list_elastic_ips(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<ElasticIp>> {
        let rows = self.executor.query_rows(
            sql::LIST_ELASTIC_IPS,
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter()
            .map(|row| decode_elastic_ip(&row))
            .collect()
    }

    fn list_all_elastic_ips(&self, page: Page) -> StorageResult<Vec<ElasticIp>> {
        let rows = self.executor.query_rows(
            sql::LIST_ALL_ELASTIC_IPS,
            &[page.limit.to_string(), page.offset.to_string()],
        )?;
        rows.into_iter()
            .map(|row| decode_elastic_ip(&row))
            .collect()
    }

    fn upsert_subject(&self, subject: &Subject) -> StorageResult<()> {
        self.executor.exec(
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
        )?;
        Ok(())
    }

    fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>> {
        let rows = self
            .executor
            .query_rows(sql::GET_SUBJECT, &[subject_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_subject(&row))
            .transpose()?)
    }

    fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>> {
        let rows = self.executor.query_rows(
            "SELECT subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active, created_at, updated_at FROM subject WHERE tenant_id = NULLIF($1, '')::uuid ORDER BY display_name ASC, subject_id ASC LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4",
            &[
                tenant_id.0.to_string(),
                page.limit.to_string(),
                page.offset.to_string(),
            ],
        )?;
        rows.into_iter().map(|row| decode_subject(&row)).collect()
    }
}
