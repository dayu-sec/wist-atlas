use atlas_domain::{
    EffectiveResponsibilityView, HostProcessGroupView, HostProcessGroupsPageView,
    HostProcessOverviewView, HostServiceInstanceView, HostServiceView, HostSummaryView,
    HostTopologyView, NetworkTopologyView, ProcessRuntimeState, ProcessStateCount,
    RuntimeObjectType, ServiceSummaryView,
};
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    Page, RuntimeStore,
};
use chrono::Utc;
use orion_error::conversion::ConvErr;
use uuid::Uuid;

use crate::error::ApiResult;

pub struct TopologyQueryService<S> {
    store: S,
}

impl<S> TopologyQueryService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> TopologyQueryService<S>
where
    S: CatalogStore + RuntimeStore + GovernanceStore,
{
    /// 主机列表：选主机用的最小集合。
    ///
    /// `include_stale` 默认不开 —— 已退役的主机保留行是为了历史，不该出现在
    /// 「现在有哪几台机器」里。
    pub fn host_summaries(
        &self,
        page: Page,
        include_stale: bool,
    ) -> ApiResult<Vec<HostSummaryView>> {
        let hosts = self.store.list_all_hosts(page).conv_err()?;
        let counts: std::collections::BTreeMap<Uuid, i64> = list_all_process_counts(&self.store)
            .conv_err()?
            .into_iter()
            .collect();

        let mut views = Vec::with_capacity(hosts.len());
        for host in hosts {
            if !include_stale && host.stale_at.is_some() {
                continue;
            }
            let segment_count = self
                .store
                .list_host_net_assocs(host.host_id, Page::default())
                .conv_err()?
                .len();
            views.push(HostSummaryView {
                process_count: counts.get(&host.host_id).copied().unwrap_or(0),
                segment_count,
                host,
            });
        }
        Ok(views)
    }

    pub fn host_topology_view(&self, host_id: Uuid) -> ApiResult<Option<HostTopologyView>> {
        let host = self.store.get_host(host_id).conv_err()?;
        let Some(host) = host else {
            return Ok(None);
        };

        let network_assocs = self
            .store
            .list_host_net_assocs(host_id, Page::default())
            .conv_err()?;
        let mut network_segments = Vec::new();
        for assoc in &network_assocs {
            if let Some(segment) = self
                .store
                .get_network_segment(assoc.network_segment_id)
                .conv_err()?
            {
                network_segments.push(segment);
            }
        }

        let assignments = self
            .store
            .list_responsibility_assignments_for_target(
                atlas_domain::ObjectKind::Host,
                host.host_id,
                Page::default(),
            )
            .conv_err()?;
        let processes = list_all_process_runtime_states(&self.store, host.host_id)?;
        let services = build_host_services(&self.store, &processes)?;
        let latest_runtime = self
            .store
            .list_host_runtime_states(host.host_id, Page::default())
            .conv_err()?
            .into_iter()
            .max_by_key(|state| state.observed_at.0);
        let containers = list_all_containers_for_host(&self.store, host.host_id)?;

        Ok(Some(HostTopologyView {
            host,
            latest_runtime,
            process_groups: build_process_groups(&processes),
            processes,
            network_segments,
            network_assocs,
            services,
            containers,
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub fn host_process_topology_view(&self, host_id: Uuid) -> ApiResult<Option<HostTopologyView>> {
        self.host_topology_view(host_id)
    }

    pub fn host_process_overview_view(
        &self,
        host_id: Uuid,
        top_n: usize,
    ) -> ApiResult<Option<HostProcessOverviewView>> {
        let Some(view) = self.host_topology_view(host_id)? else {
            return Ok(None);
        };
        Ok(Some(build_host_process_overview_view(view, top_n)))
    }

    pub fn host_process_groups_page_view(
        &self,
        host_id: Uuid,
        offset: usize,
        limit: usize,
    ) -> ApiResult<Option<HostProcessGroupsPageView>> {
        let Some(view) = self.host_topology_view(host_id)? else {
            return Ok(None);
        };
        Ok(Some(build_host_process_groups_page_view(
            view, offset, limit,
        )))
    }

    pub fn network_topology_view(
        &self,
        network_segment_id: Uuid,
    ) -> ApiResult<Option<NetworkTopologyView>> {
        let segment = self
            .store
            .get_network_segment(network_segment_id)
            .conv_err()?;
        let Some(segment) = segment else {
            return Ok(None);
        };

        let mut hosts = Vec::new();
        let mut host_assocs = Vec::new();
        for host in self
            .store
            .list_hosts(segment.tenant_id, Page::default())
            .conv_err()?
        {
            let assocs = self
                .store
                .list_host_net_assocs(host.host_id, Page::default())
                .conv_err()?;
            let matched: Vec<_> = assocs
                .into_iter()
                .filter(|assoc| assoc.network_segment_id == network_segment_id)
                .collect();
            if !matched.is_empty() {
                hosts.push(host);
                host_assocs.extend(matched);
            }
        }

        let assignments = self
            .store
            .list_responsibility_assignments_for_target(
                atlas_domain::ObjectKind::NetworkSegment,
                network_segment_id,
                Page::default(),
            )
            .conv_err()?;

        Ok(Some(NetworkTopologyView {
            segment,
            hosts,
            host_assocs,
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub fn effective_responsibility_view(
        &self,
        target_kind: atlas_domain::ObjectKind,
        target_id: Uuid,
    ) -> ApiResult<Vec<EffectiveResponsibilityView>> {
        let assignments = self
            .store
            .list_responsibility_assignments_for_target(target_kind, target_id, Page::default())
            .conv_err()?;

        let mut views = Vec::new();
        for assignment in assignments {
            if let Some(subject) = self.store.get_subject(assignment.subject_id).conv_err()? {
                views.push(EffectiveResponsibilityView {
                    subject,
                    assignment,
                    generated_at: Utc::now(),
                });
            }
        }

        Ok(views)
    }
}

impl<S> TopologyQueryService<S>
where
    S: AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    pub async fn host_summaries_async(
        &self,
        page: Page,
        include_stale: bool,
    ) -> ApiResult<Vec<HostSummaryView>> {
        let hosts = AsyncCatalogStore::list_all_hosts(&self.store, page)
            .await
            .conv_err()?;
        let counts: std::collections::BTreeMap<Uuid, i64> =
            list_all_process_counts_async(&self.store)
                .await
                .conv_err()?
                .into_iter()
                .collect();

        let mut views = Vec::with_capacity(hosts.len());
        for host in hosts {
            if !include_stale && host.stale_at.is_some() {
                continue;
            }
            let segment_count =
                AsyncRuntimeStore::list_host_net_assocs(&self.store, host.host_id, Page::default())
                    .await
                    .conv_err()?
                    .len();
            views.push(HostSummaryView {
                process_count: counts.get(&host.host_id).copied().unwrap_or(0),
                segment_count,
                host,
            });
        }
        Ok(views)
    }

    /// 单租户部署下的「当前租户」：取第一台主机所属的租户。
    ///
    /// 多租户路由还没做，HTTP 层也没有租户上下文。与其在接口上加一个必填 query
    /// 参数让前端去猜，不如按现有数据回答 —— 库里只有一个租户时这就是它。
    pub async fn default_tenant_id_async(&self) -> ApiResult<Option<atlas_domain::TenantId>> {
        let hosts = AsyncCatalogStore::list_all_hosts(
            &self.store,
            Page {
                limit: 1,
                offset: 0,
            },
        )
        .await
        .conv_err()?;
        Ok(hosts.into_iter().next().map(|host| host.tenant_id))
    }

    /// 服务列表：哪个服务铺得最广。
    ///
    /// 端点明细不放进来（那属于服务焦点视图）；这里只回答「有多少实例、落在几台主机」。
    /// 实例→主机靠 `live_instance_hosts_async` 按主机反查一次，不逐实例查 binding。
    pub async fn service_summaries_async(
        &self,
        tenant_id: atlas_domain::TenantId,
        page: Page,
    ) -> ApiResult<Vec<ServiceSummaryView>> {
        let services = AsyncCatalogStore::list_services(&self.store, tenant_id, page)
            .await
            .conv_err()?;
        let host_by_instance = live_instance_hosts_async(&self.store).await?;

        let mut views = Vec::with_capacity(services.len());
        for service in services {
            let instances =
                list_all_service_instances_async(&self.store, service.service_id).await?;
            let mut hosts = std::collections::BTreeSet::new();
            let mut live_instances = 0usize;
            for instance in &instances {
                if let Some(host_id) = host_by_instance.get(&instance.instance_id) {
                    live_instances += 1;
                    hosts.insert(*host_id);
                }
            }
            views.push(ServiceSummaryView {
                service,
                instance_count: live_instances,
                host_count: hosts.len(),
            });
        }

        // 铺得广的排前面：这个列表的用途是「先看哪套服务动了好几台机」。
        views.sort_by(|left, right| {
            right
                .host_count
                .cmp(&left.host_count)
                .then(right.instance_count.cmp(&left.instance_count))
                .then(left.service.name.cmp(&right.service.name))
        });
        Ok(views)
    }

    pub async fn host_topology_view_async(
        &self,
        host_id: Uuid,
    ) -> ApiResult<Option<HostTopologyView>> {
        let host = AsyncCatalogStore::get_host(&self.store, host_id)
            .await
            .conv_err()?;
        let Some(host) = host else {
            return Ok(None);
        };

        let network_assocs =
            AsyncRuntimeStore::list_host_net_assocs(&self.store, host_id, Page::default())
                .await
                .conv_err()?;
        let mut network_segments = Vec::new();
        for assoc in &network_assocs {
            if let Some(segment) =
                AsyncCatalogStore::get_network_segment(&self.store, assoc.network_segment_id)
                    .await
                    .conv_err()?
            {
                network_segments.push(segment);
            }
        }

        let assignments = AsyncGovernanceStore::list_responsibility_assignments_for_target(
            &self.store,
            atlas_domain::ObjectKind::Host,
            host.host_id,
            Page::default(),
        )
        .await
        .conv_err()?;
        let processes = list_all_process_runtime_states_async(&self.store, host.host_id).await?;
        let services = build_host_services_async(&self.store, &processes).await?;
        let latest_runtime =
            AsyncRuntimeStore::list_host_runtime_states(&self.store, host.host_id, Page::default())
                .await
                .conv_err()?
                .into_iter()
                .max_by_key(|state| state.observed_at.0);
        let containers = list_all_containers_for_host_async(&self.store, host.host_id).await?;

        Ok(Some(HostTopologyView {
            host,
            latest_runtime,
            process_groups: build_process_groups(&processes),
            processes,
            network_segments,
            network_assocs,
            services,
            containers,
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub async fn host_process_topology_view_async(
        &self,
        host_id: Uuid,
    ) -> ApiResult<Option<HostTopologyView>> {
        self.host_topology_view_async(host_id).await
    }

    pub async fn host_process_overview_view_async(
        &self,
        host_id: Uuid,
        top_n: usize,
    ) -> ApiResult<Option<HostProcessOverviewView>> {
        let Some(view) = self.host_topology_view_async(host_id).await? else {
            return Ok(None);
        };
        Ok(Some(build_host_process_overview_view(view, top_n)))
    }

    pub async fn host_process_groups_page_view_async(
        &self,
        host_id: Uuid,
        offset: usize,
        limit: usize,
    ) -> ApiResult<Option<HostProcessGroupsPageView>> {
        let Some(view) = self.host_topology_view_async(host_id).await? else {
            return Ok(None);
        };
        Ok(Some(build_host_process_groups_page_view(
            view, offset, limit,
        )))
    }

    pub async fn network_topology_view_async(
        &self,
        network_segment_id: Uuid,
    ) -> ApiResult<Option<NetworkTopologyView>> {
        let segment = AsyncCatalogStore::get_network_segment(&self.store, network_segment_id)
            .await
            .conv_err()?;
        let Some(segment) = segment else {
            return Ok(None);
        };

        let mut hosts = Vec::new();
        let mut host_assocs = Vec::new();
        for host in AsyncCatalogStore::list_hosts(&self.store, segment.tenant_id, Page::default())
            .await
            .conv_err()?
        {
            let assocs =
                AsyncRuntimeStore::list_host_net_assocs(&self.store, host.host_id, Page::default())
                    .await
                    .conv_err()?;
            let matched: Vec<_> = assocs
                .into_iter()
                .filter(|assoc| assoc.network_segment_id == network_segment_id)
                .collect();
            if !matched.is_empty() {
                hosts.push(host);
                host_assocs.extend(matched);
            }
        }

        let assignments = AsyncGovernanceStore::list_responsibility_assignments_for_target(
            &self.store,
            atlas_domain::ObjectKind::NetworkSegment,
            network_segment_id,
            Page::default(),
        )
        .await
        .conv_err()?;

        Ok(Some(NetworkTopologyView {
            segment,
            hosts,
            host_assocs,
            assignments,
            generated_at: Utc::now(),
        }))
    }

    pub async fn effective_responsibility_view_async(
        &self,
        target_kind: atlas_domain::ObjectKind,
        target_id: Uuid,
    ) -> ApiResult<Vec<EffectiveResponsibilityView>> {
        let assignments = AsyncGovernanceStore::list_responsibility_assignments_for_target(
            &self.store,
            target_kind,
            target_id,
            Page::default(),
        )
        .await
        .conv_err()?;

        let mut views = Vec::new();
        for assignment in assignments {
            if let Some(subject) =
                AsyncCatalogStore::get_subject(&self.store, assignment.subject_id)
                    .await
                    .conv_err()?
            {
                views.push(EffectiveResponsibilityView {
                    subject,
                    assignment,
                    generated_at: Utc::now(),
                });
            }
        }

        Ok(views)
    }
}

mod helpers;
use helpers::{
    build_host_process_groups_page_view, build_host_process_overview_view, build_host_services,
    build_host_services_async, build_process_groups, list_all_containers_for_host,
    list_all_containers_for_host_async, list_all_process_runtime_states,
    list_all_service_instances_async, live_instance_hosts_async,
};
// 焦点图的组装在 `atlas-app`，但「一台机的现役进程」这条读取语义只该有一份实现，
// 所以在 crate 根上重新导出，避免两边各写一遍 stale 过滤和翻页口径。
pub use helpers::list_all_process_runtime_states_async;

/// 一次取多少个主机的进程计数。主机数通常个位数，但这个查询也要翻页 ——
/// 只取第一页会让第 101 台之后的主机在列表里显示成「0 进程」。
const HOST_COUNT_PAGE_LIMIT: u32 = 200;

fn list_all_process_counts<S>(store: &S) -> atlas_storage::StorageResult<Vec<(Uuid, i64)>>
where
    S: RuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_host_process_counts(Page {
            limit: HOST_COUNT_PAGE_LIMIT,
            offset,
        })?;
        let len = batch.len();
        items.extend(batch);
        if len < HOST_COUNT_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

async fn list_all_process_counts_async<S>(
    store: &S,
) -> atlas_storage::StorageResult<Vec<(Uuid, i64)>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_host_process_counts(
            store,
            Page {
                limit: HOST_COUNT_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let len = batch.len();
        items.extend(batch);
        if len < HOST_COUNT_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}
