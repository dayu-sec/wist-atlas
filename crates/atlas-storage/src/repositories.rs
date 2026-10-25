use atlas_domain::{
    BusinessDomain, ClusterInventory, ContainerRuntime, ExternalIdentityLink, ExternalSyncCursor,
    HostInventory, HostNetAssoc, HostRuntimeState, InstanceEndpoint, NamespaceInventory,
    NetworkDomain, NetworkSegment, ObjectKind, PodInventory, PodPlacement, ProcessRuntimeState,
    ResponsibilityAssignment, RuntimeBinding, ServiceEntity, ServiceInstance, Subject, Subsystem,
    SystemBoundary, TenantId, WorkloadEntity, WorkloadPodMembership,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::StorageResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    pub limit: u32,
    pub offset: u32,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            limit: 100,
            offset: 0,
        }
    }
}

pub trait CatalogStore {
    fn upsert_business(&self, business: &BusinessDomain) -> StorageResult<()>;
    fn get_business(&self, business_id: Uuid) -> StorageResult<Option<BusinessDomain>>;
    fn list_businesses(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<BusinessDomain>>;

    fn upsert_system(&self, system: &SystemBoundary) -> StorageResult<()>;
    fn get_system(&self, system_id: Uuid) -> StorageResult<Option<SystemBoundary>>;

    fn upsert_subsystem(&self, subsystem: &Subsystem) -> StorageResult<()>;
    fn get_subsystem(&self, subsystem_id: Uuid) -> StorageResult<Option<Subsystem>>;

    fn upsert_service(&self, service: &ServiceEntity) -> StorageResult<()>;
    fn get_service(&self, service_id: Uuid) -> StorageResult<Option<ServiceEntity>>;
    fn list_services(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<ServiceEntity>>;

    fn upsert_cluster(&self, cluster: &ClusterInventory) -> StorageResult<()>;
    fn get_cluster(&self, cluster_id: Uuid) -> StorageResult<Option<ClusterInventory>>;

    fn upsert_namespace(&self, namespace: &NamespaceInventory) -> StorageResult<()>;
    fn get_namespace(&self, namespace_id: Uuid) -> StorageResult<Option<NamespaceInventory>>;

    fn upsert_workload(&self, workload: &WorkloadEntity) -> StorageResult<()>;
    fn get_workload(&self, workload_id: Uuid) -> StorageResult<Option<WorkloadEntity>>;

    fn upsert_pod(&self, pod: &PodInventory) -> StorageResult<()>;
    fn get_pod(&self, pod_id: Uuid) -> StorageResult<Option<PodInventory>>;

    fn upsert_host(&self, host: &HostInventory) -> StorageResult<()>;
    fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>>;
    fn list_hosts(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<HostInventory>>;
    fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>>;

    fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()>;
    fn get_network_domain(&self, network_domain_id: Uuid) -> StorageResult<Option<NetworkDomain>>;

    fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()>;
    fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>>;
    fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>>;

    fn upsert_subject(&self, subject: &Subject) -> StorageResult<()>;
    fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>>;
    fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncCatalogStore: Sync {
    async fn upsert_business(&self, business: &BusinessDomain) -> StorageResult<()>;
    async fn get_business(&self, business_id: Uuid) -> StorageResult<Option<BusinessDomain>>;
    async fn list_businesses(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<BusinessDomain>>;
    async fn upsert_system(&self, system: &SystemBoundary) -> StorageResult<()>;
    async fn get_system(&self, system_id: Uuid) -> StorageResult<Option<SystemBoundary>>;
    async fn upsert_subsystem(&self, subsystem: &Subsystem) -> StorageResult<()>;
    async fn get_subsystem(&self, subsystem_id: Uuid) -> StorageResult<Option<Subsystem>>;
    async fn upsert_service(&self, service: &ServiceEntity) -> StorageResult<()>;
    async fn get_service(&self, service_id: Uuid) -> StorageResult<Option<ServiceEntity>>;
    async fn list_services(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<ServiceEntity>>;
    async fn upsert_cluster(&self, cluster: &ClusterInventory) -> StorageResult<()>;
    async fn get_cluster(&self, cluster_id: Uuid) -> StorageResult<Option<ClusterInventory>>;
    async fn upsert_namespace(&self, namespace: &NamespaceInventory) -> StorageResult<()>;
    async fn get_namespace(&self, namespace_id: Uuid) -> StorageResult<Option<NamespaceInventory>>;
    async fn upsert_workload(&self, workload: &WorkloadEntity) -> StorageResult<()>;
    async fn get_workload(&self, workload_id: Uuid) -> StorageResult<Option<WorkloadEntity>>;
    async fn upsert_pod(&self, pod: &PodInventory) -> StorageResult<()>;
    async fn get_pod(&self, pod_id: Uuid) -> StorageResult<Option<PodInventory>>;
    async fn upsert_host(&self, host: &HostInventory) -> StorageResult<()>;
    async fn get_host(&self, host_id: Uuid) -> StorageResult<Option<HostInventory>>;
    async fn list_hosts(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<HostInventory>>;
    async fn list_all_hosts(&self, page: Page) -> StorageResult<Vec<HostInventory>>;
    async fn upsert_network_domain(&self, domain: &NetworkDomain) -> StorageResult<()>;
    async fn get_network_domain(
        &self,
        network_domain_id: Uuid,
    ) -> StorageResult<Option<NetworkDomain>>;
    async fn upsert_network_segment(&self, segment: &NetworkSegment) -> StorageResult<()>;
    async fn get_network_segment(
        &self,
        network_segment_id: Uuid,
    ) -> StorageResult<Option<NetworkSegment>>;
    async fn list_network_segments(
        &self,
        tenant_id: TenantId,
        page: Page,
    ) -> StorageResult<Vec<NetworkSegment>>;
    async fn upsert_subject(&self, subject: &Subject) -> StorageResult<()>;
    async fn get_subject(&self, subject_id: Uuid) -> StorageResult<Option<Subject>>;
    async fn list_subjects(&self, tenant_id: TenantId, page: Page) -> StorageResult<Vec<Subject>>;
}

pub trait RuntimeStore {
    fn insert_host_runtime_state(&self, state: &HostRuntimeState) -> StorageResult<()>;
    fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostRuntimeState>>;

    fn upsert_process_runtime_state(&self, state: &ProcessRuntimeState) -> StorageResult<()>;
    fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ProcessRuntimeState>>;

    fn upsert_container_runtime(&self, container: &ContainerRuntime) -> StorageResult<()>;
    /// 这台主机**当前活着**的容器（`stale_at IS NULL`）。
    ///
    /// 退役行留在表里供回溯，但不能进现役视图 —— 与进程、端口的读侧口径一致。
    fn list_container_runtimes_for_host(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ContainerRuntime>>;

    /// 每台主机当前的存活进程数（`stale_at IS NULL`），已按 `host_id` 排序。
    ///
    /// 主机列表页要显示每台机的规模；让前端逐台翻页数进程会把一页列表变成
    /// N 次查询，而这是 `process_runtime_state` 上一次 GROUP BY 就能算完的事。
    /// 没有任何存活进程的主机不会出现在结果里（调用方按缺失当 0）。
    fn list_host_process_counts(&self, page: Page) -> StorageResult<Vec<(Uuid, i64)>>;

    fn upsert_service_instance(&self, instance: &ServiceInstance) -> StorageResult<()>;
    fn get_service_instance(&self, instance_id: Uuid) -> StorageResult<Option<ServiceInstance>>;
    fn list_service_instances(
        &self,
        service_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ServiceInstance>>;

    fn upsert_instance_endpoint(&self, endpoint: &InstanceEndpoint) -> StorageResult<()>;
    fn list_instance_endpoints(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<InstanceEndpoint>>;
    fn close_instance_endpoint(&self, endpoint_id: Uuid, at: DateTime<Utc>) -> StorageResult<()>;

    /// 这台主机上**仍然活着**（进程未标 stale）的服务实例 id，已去重。
    ///
    /// 导入「本轮没有任何端口在听」的快照时要关掉该主机的活端点；按进程逐个反查
    /// binding 在千级进程规模下会把一次导入放大成上千次查询，所以按 host 直接取。
    fn list_instance_ids_for_host(&self, host_id: Uuid) -> StorageResult<Vec<Uuid>>;

    fn upsert_runtime_binding(&self, binding: &RuntimeBinding) -> StorageResult<()>;
    fn get_runtime_binding(&self, binding_id: Uuid) -> StorageResult<Option<RuntimeBinding>>;
    fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>>;
    fn list_runtime_bindings_for_object(
        &self,
        object_type: atlas_domain::RuntimeObjectType,
        object_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>>;

    fn upsert_workload_pod_membership(
        &self,
        membership: &WorkloadPodMembership,
    ) -> StorageResult<()>;

    fn upsert_pod_placement(&self, placement: &PodPlacement) -> StorageResult<()>;

    fn upsert_host_net_assoc(&self, assoc: &HostNetAssoc) -> StorageResult<()>;
    fn list_host_net_assocs(&self, host_id: Uuid, page: Page) -> StorageResult<Vec<HostNetAssoc>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncRuntimeStore: Sync {
    async fn insert_host_runtime_state(&self, state: &HostRuntimeState) -> StorageResult<()>;
    async fn list_host_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostRuntimeState>>;
    async fn upsert_process_runtime_state(&self, state: &ProcessRuntimeState) -> StorageResult<()>;
    async fn list_process_runtime_states(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ProcessRuntimeState>>;
    async fn upsert_container_runtime(&self, container: &ContainerRuntime) -> StorageResult<()>;
    async fn list_container_runtimes_for_host(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ContainerRuntime>>;
    async fn list_host_process_counts(&self, page: Page) -> StorageResult<Vec<(Uuid, i64)>>;
    async fn upsert_service_instance(&self, instance: &ServiceInstance) -> StorageResult<()>;
    async fn get_service_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Option<ServiceInstance>>;
    async fn list_service_instances(
        &self,
        service_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ServiceInstance>>;
    async fn upsert_instance_endpoint(&self, endpoint: &InstanceEndpoint) -> StorageResult<()>;
    async fn list_instance_endpoints(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<InstanceEndpoint>>;
    async fn close_instance_endpoint(
        &self,
        endpoint_id: Uuid,
        at: DateTime<Utc>,
    ) -> StorageResult<()>;
    async fn list_instance_ids_for_host(&self, host_id: Uuid) -> StorageResult<Vec<Uuid>>;
    async fn upsert_runtime_binding(&self, binding: &RuntimeBinding) -> StorageResult<()>;
    async fn get_runtime_binding(&self, binding_id: Uuid) -> StorageResult<Option<RuntimeBinding>>;
    async fn list_runtime_bindings_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>>;
    async fn list_runtime_bindings_for_object(
        &self,
        object_type: atlas_domain::RuntimeObjectType,
        object_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<RuntimeBinding>>;
    async fn upsert_workload_pod_membership(
        &self,
        membership: &WorkloadPodMembership,
    ) -> StorageResult<()>;
    async fn upsert_pod_placement(&self, placement: &PodPlacement) -> StorageResult<()>;
    async fn upsert_host_net_assoc(&self, assoc: &HostNetAssoc) -> StorageResult<()>;
    async fn list_host_net_assocs(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<HostNetAssoc>>;
}

pub trait GovernanceStore {
    fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()>;

    fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>>;

    fn list_responsibility_assignments_for_target(
        &self,
        target_kind: ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncGovernanceStore: Sync {
    async fn upsert_responsibility_assignment(
        &self,
        assignment: &ResponsibilityAssignment,
    ) -> StorageResult<()>;
    async fn get_responsibility_assignment(
        &self,
        assignment_id: Uuid,
    ) -> StorageResult<Option<ResponsibilityAssignment>>;
    async fn list_responsibility_assignments_for_target(
        &self,
        target_kind: ObjectKind,
        target_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<ResponsibilityAssignment>>;
}

pub trait SyncStore {
    fn upsert_external_identity_link(&self, link: &ExternalIdentityLink) -> StorageResult<()>;
    fn get_external_identity_link(
        &self,
        link_id: Uuid,
    ) -> StorageResult<Option<ExternalIdentityLink>>;

    fn upsert_external_sync_cursor(&self, cursor: &ExternalSyncCursor) -> StorageResult<()>;
    fn get_external_sync_cursor(
        &self,
        cursor_id: Uuid,
    ) -> StorageResult<Option<ExternalSyncCursor>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncSyncStore: Sync {
    async fn upsert_external_identity_link(&self, link: &ExternalIdentityLink)
    -> StorageResult<()>;
    async fn get_external_identity_link(
        &self,
        link_id: Uuid,
    ) -> StorageResult<Option<ExternalIdentityLink>>;
    async fn upsert_external_sync_cursor(&self, cursor: &ExternalSyncCursor) -> StorageResult<()>;
    async fn get_external_sync_cursor(
        &self,
        cursor_id: Uuid,
    ) -> StorageResult<Option<ExternalSyncCursor>>;
}
