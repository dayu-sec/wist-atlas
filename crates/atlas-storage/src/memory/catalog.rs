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

impl CatalogStore for InMemoryTopologyStore {
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
        self.with_state(|state| {
            if let Some(existing) = state
                .services
                .iter_mut()
                .find(|item| item.service_id == service.service_id)
            {
                *existing = service.clone();
            } else {
                state.services.push(service.clone());
            }
        })
    }

    fn get_service(&self, service_id: Uuid) -> StorageResult<Option<atlas_domain::ServiceEntity>> {
        self.with_state(|state| {
            state
                .services
                .iter()
                .find(|item| item.service_id == service_id)
                .cloned()
        })
    }

    fn list_services(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<atlas_domain::ServiceEntity>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .services
                .iter()
                .filter(|item| item.tenant_id == tenant_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
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
        self.with_state(|state| {
            if let Some(existing) = state
                .hosts
                .iter_mut()
                .find(|item| item.host_id == host.host_id)
            {
                *existing = host.clone();
            } else {
                state.hosts.push(host.clone());
            }
        })
    }

    fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>> {
        self.with_state(|state| {
            state
                .hosts
                .iter()
                .find(|item| item.host_id == host_id)
                .cloned()
        })
    }

    fn list_hosts(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<HostInventory>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .hosts
                .iter()
                .filter(|item| item.tenant_id == tenant_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            let mut hosts = state.hosts.clone();
            hosts.sort_by(|left, right| {
                left.host_name
                    .cmp(&right.host_name)
                    .then(left.host_id.cmp(&right.host_id))
            });
            hosts
                .into_iter()
                .skip(start)
                .take(page.limit as usize)
                .collect()
        })
    }

    fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .network_domains
                .iter_mut()
                .find(|item| item.network_domain_id == domain.network_domain_id)
            {
                *existing = domain.clone();
            } else {
                state.network_domains.push(domain.clone());
            }
        })
    }

    fn get_network_domain(&self, network_domain_id: Uuid) -> StorageResult<Option<NetworkDomain>> {
        self.with_state(|state| {
            state
                .network_domains
                .iter()
                .find(|item| item.network_domain_id == network_domain_id)
                .cloned()
        })
    }

    fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .network_segments
                .iter_mut()
                .find(|item| item.network_segment_id == segment.network_segment_id)
            {
                *existing = segment.clone();
            } else {
                state.network_segments.push(segment.clone());
            }
        })
    }

    fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>> {
        self.with_state(|state| {
            state
                .network_segments
                .iter()
                .find(|item| item.network_segment_id == network_segment_id)
                .cloned()
        })
    }

    fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .network_segments
                .iter()
                .filter(|item| item.tenant_id == tenant_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn upsert_subject(&self, subject: &Subject) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .subjects
                .iter_mut()
                .find(|item| item.subject_id == subject.subject_id)
            {
                *existing = subject.clone();
            } else {
                state.subjects.push(subject.clone());
            }
        })
    }

    fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>> {
        self.with_state(|state| {
            state
                .subjects
                .iter()
                .find(|item| item.subject_id == subject_id)
                .cloned()
        })
    }

    fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>> {
        self.with_state(|state| {
            let start = page.offset as usize;
            state
                .subjects
                .iter()
                .filter(|item| item.tenant_id == tenant_id)
                .skip(start)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }
}
