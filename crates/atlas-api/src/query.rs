use atlas_domain::{
    EffectiveResponsibilityView, HostProcessGroupView, HostProcessGroupsPageView,
    HostProcessOverviewView, HostServiceInstanceView, HostServiceView, HostTopologyView,
    NetworkTopologyView, ProcessRuntimeState, ProcessStateCount, RuntimeObjectType,
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

        Ok(Some(HostTopologyView {
            host,
            latest_runtime,
            process_groups: build_process_groups(&processes),
            processes,
            network_segments,
            network_assocs,
            services,
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

        Ok(Some(HostTopologyView {
            host,
            latest_runtime,
            process_groups: build_process_groups(&processes),
            processes,
            network_segments,
            network_assocs,
            services,
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
    build_host_services_async, build_process_groups, list_all_process_runtime_states,
    list_all_process_runtime_states_async,
};
