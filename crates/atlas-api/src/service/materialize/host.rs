use std::collections::BTreeSet;

use atlas_domain::{
    AgentHealth, BindingScope, BusinessCatalogCandidate, Confidence, HostCandidate, HostInventory,
    HostRuntimeState, HostTelemetryCandidate, NetworkSegmentCandidate, ProcessRuntimeCandidate,
    ProcessRuntimeState, ProcessTelemetryCandidate, ResponsibilityAssignment, RuntimeBinding,
    RuntimeObjectType, ServiceEntity, ServiceInstance, Subject, SubjectCandidate, TenantId,
    ValidityWindow,
};
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore,
    InMemoryTopologyStore, Page, RuntimeStore, StorageResult,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{find_service_by_ref, find_service_by_ref_async, stable_uuid};
use crate::pipeline::{InMemoryCatalog, materialize_host_network, resolve_host_candidate};

/// 一次主机/网段物化的结果。
///
/// `host_ids` 是主机的「本次快照覆盖集」，交给退役清扫用 —— 只知道数量是没法比对
/// 「谁没出现」的。
#[derive(Debug, Default, Clone)]
pub(crate) struct MaterializedHosts {
    pub host_ids: BTreeSet<Uuid>,
    pub host_count: usize,
    pub network_count: usize,
    pub assoc_count: usize,
}

pub(crate) fn materialize_candidates<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    hosts: Vec<HostCandidate>,
    networks: Vec<NetworkSegmentCandidate>,
    now: DateTime<Utc>,
    scope: &str,
) -> StorageResult<MaterializedHosts>
where
    S: CatalogStore + RuntimeStore,
{
    let mut result = MaterializedHosts::default();

    for network_candidate in networks {
        let host_candidate =
            resolve_host_candidate_for_network(&hosts, catalog, &network_candidate);
        if let Some(host_candidate) = host_candidate {
            let materialized =
                materialize_host_network(catalog, &host_candidate, &network_candidate, now);

            for domain in &catalog.network_domains {
                store.upsert_network_domain(domain)?;
            }
            let mut host = materialized.host;
            mark_seen(&mut host, &host_candidate, scope);
            upsert_catalog_host(catalog, host.clone());
            store.upsert_host(&host)?;
            store.upsert_network_segment(&materialized.segment)?;
            if let Some(assoc) = &materialized.assoc {
                store.upsert_host_net_assoc(assoc)?;
                result.assoc_count += 1;
            }
            result.host_ids.insert(host.host_id);
            result.host_count += 1;
            result.network_count += 1;
        }
    }

    for host_candidate in hosts {
        let host_resolution = resolve_host_candidate(catalog, &host_candidate, now);
        if result.host_ids.contains(&host_resolution.host.host_id) {
            continue;
        }

        let mut host = host_resolution.host;
        mark_seen(&mut host, &host_candidate, scope);
        upsert_catalog_host(catalog, host.clone());
        store.upsert_host(&host)?;
        result.host_ids.insert(host.host_id);
        result.host_count += 1;
    }

    Ok(result)
}

pub(crate) async fn materialize_candidates_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    hosts: Vec<HostCandidate>,
    networks: Vec<NetworkSegmentCandidate>,
    now: DateTime<Utc>,
    scope: &str,
) -> StorageResult<MaterializedHosts>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let mut result = MaterializedHosts::default();

    for network_candidate in networks {
        let host_candidate =
            resolve_host_candidate_for_network(&hosts, catalog, &network_candidate);
        if let Some(host_candidate) = host_candidate {
            let materialized =
                materialize_host_network(catalog, &host_candidate, &network_candidate, now);

            for domain in &catalog.network_domains {
                atlas_storage::AsyncCatalogStore::upsert_network_domain(store, domain).await?;
            }
            let mut host = materialized.host;
            mark_seen(&mut host, &host_candidate, scope);
            upsert_catalog_host(catalog, host.clone());
            atlas_storage::AsyncCatalogStore::upsert_host(store, &host).await?;
            atlas_storage::AsyncCatalogStore::upsert_network_segment(store, &materialized.segment)
                .await?;
            if let Some(assoc) = &materialized.assoc {
                atlas_storage::AsyncRuntimeStore::upsert_host_net_assoc(store, assoc).await?;
                result.assoc_count += 1;
            }
            result.host_ids.insert(host.host_id);
            result.host_count += 1;
            result.network_count += 1;
        }
    }

    for host_candidate in hosts {
        let host_resolution = resolve_host_candidate(catalog, &host_candidate, now);
        if result.host_ids.contains(&host_resolution.host.host_id) {
            continue;
        }

        let mut host = host_resolution.host;
        mark_seen(&mut host, &host_candidate, scope);
        upsert_catalog_host(catalog, host.clone());
        atlas_storage::AsyncCatalogStore::upsert_host(store, &host).await?;
        result.host_ids.insert(host.host_id);
        result.host_count += 1;
    }

    Ok(result)
}

pub(crate) fn resolve_host_candidate_for_network(
    hosts: &[HostCandidate],
    catalog: &InMemoryCatalog,
    network_candidate: &NetworkSegmentCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = network_candidate.machine_id.as_ref() {
        if let Some(host) = hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(host.clone());
        }

        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: network_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
            });
        }
    }

    if let Some(host_name) = network_candidate.host_name.as_ref() {
        if let Some(host) = hosts.iter().find(|host| &host.host_name == host_name) {
            return Some(host.clone());
        }

        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: network_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
            });
        }
    }

    None
}

pub(crate) fn resolve_host_candidate_for_process(
    catalog: &InMemoryCatalog,
    process_candidate: &ProcessRuntimeCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = process_candidate.machine_id.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: process_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
            });
        }
    }

    if let Some(host_name) = process_candidate.host_name.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: process_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
            });
        }
    }

    None
}

pub(crate) fn upsert_catalog_host(
    catalog: &mut InMemoryCatalog,
    host: atlas_domain::HostInventory,
) {
    if let Some(existing) = catalog
        .hosts
        .iter_mut()
        .find(|existing| existing.host_id == host.host_id)
    {
        *existing = host;
    } else {
        catalog.hosts.push(host);
    }
}

/// 把「本次快照看到了它」写进这条主机记录。
///
/// 四件事：架构取候选值（候选没有就保留已知值，不用空值擦掉）、作用域归属、缺失计数归零、
/// 清掉 stale —— 也就是「重新出现即复活」。
///
/// `stale_at` 只由清扫写（连续缺失达到阈值才写），这里一律写 `None`。
fn mark_seen(host: &mut HostInventory, candidate: &HostCandidate, scope: &str) {
    if candidate.arch.is_some() {
        host.arch = candidate.arch.clone();
    }
    host.stale_at = None;
    host.miss_count = 0;
    host.ingest_scope = Some(scope.to_string());
}

/// 连续多少份**完整快照**没看到一台主机之后，判定它退役。
///
/// 为什么不是「一份没看到就退役」：主机消失可能只是一轮探针失败或短暂离线，
/// 而误标一台活主机会让它从资产目录里消失（误判代价远大于晚几轮才标）。3 份 × 默认 5 分钟
/// ≈ 15 分钟，足够滤掉抖动。
pub(crate) const HOST_STALE_AFTER_MISSES: i64 = 3;

/// 一次主机退役清扫的结果。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostStaleSweep {
    /// 本轮**新**判定退役的主机数（已达阈值且此前未标）。
    pub newly_stale: u64,
    /// 本轮被判为「缺失」的主机数（含还没达到阈值的）。
    pub missing: u64,
    /// 本次快照里没有主机 → 放弃清扫（空不等于「全没了」）。
    pub skipped_empty: bool,
}

/// 主机退役清扫：本次快照没出现的主机累加缺失计数，达到阈值才标 `stale_at`。
///
/// 与进程清扫的三点不同（都不是可选项）：
///
/// 1. **按阈值、不按单次**。见 [`HOST_STALE_AFTER_MISSES`]。
/// 2. **已退役的冻结**：`stale_at` 保留首次判定时间，也不再继续累加计数。
/// 3. **只动同作用域的行**：同租户其它生产者报的主机不受影响。存量行没有作用域
///    （升级前的数据）就不参与清扫 —— 宁可不标，也不要误标。
pub(crate) fn mark_stale_hosts<S>(
    store: &S,
    tenant_id: TenantId,
    seen: &BTreeSet<Uuid>,
    scope: &str,
    at: DateTime<Utc>,
) -> StorageResult<HostStaleSweep>
where
    S: CatalogStore,
{
    if seen.is_empty() {
        return Ok(HostStaleSweep {
            skipped_empty: true,
            ..HostStaleSweep::default()
        });
    }

    let mut result = HostStaleSweep::default();
    for mut host in list_hosts_paged(store, tenant_id)? {
        if host.ingest_scope.as_deref() != Some(scope) {
            continue;
        }
        if seen.contains(&host.host_id) || host.stale_at.is_some() {
            continue;
        }

        host.miss_count += 1;
        result.missing += 1;
        if host.miss_count >= HOST_STALE_AFTER_MISSES {
            host.stale_at = Some(at);
            result.newly_stale += 1;
        }
        store.upsert_host(&host)?;
    }

    Ok(result)
}

/// 同 [`mark_stale_hosts`]。
pub(crate) async fn mark_stale_hosts_async<S>(
    store: &S,
    tenant_id: TenantId,
    seen: &BTreeSet<Uuid>,
    scope: &str,
    at: DateTime<Utc>,
) -> StorageResult<HostStaleSweep>
where
    S: AsyncCatalogStore,
{
    if seen.is_empty() {
        return Ok(HostStaleSweep {
            skipped_empty: true,
            ..HostStaleSweep::default()
        });
    }

    let mut result = HostStaleSweep::default();
    for mut host in list_hosts_paged_async(store, tenant_id).await? {
        if host.ingest_scope.as_deref() != Some(scope) {
            continue;
        }
        if seen.contains(&host.host_id) || host.stale_at.is_some() {
            continue;
        }

        host.miss_count += 1;
        result.missing += 1;
        if host.miss_count >= HOST_STALE_AFTER_MISSES {
            host.stale_at = Some(at);
            result.newly_stale += 1;
        }
        AsyncCatalogStore::upsert_host(store, &host).await?;
    }

    Ok(result)
}

const HOST_PAGE_LIMIT: u32 = 500;

fn list_hosts_paged<S>(store: &S, tenant_id: TenantId) -> StorageResult<Vec<HostInventory>>
where
    S: CatalogStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_hosts(
            tenant_id,
            Page {
                limit: HOST_PAGE_LIMIT,
                offset,
            },
        )?;
        let batch_len = batch.len();
        items.extend(batch);
        if batch_len < HOST_PAGE_LIMIT as usize {
            break;
        }
        offset += batch_len as u32;
    }
    Ok(items)
}

async fn list_hosts_paged_async<S>(
    store: &S,
    tenant_id: TenantId,
) -> StorageResult<Vec<HostInventory>>
where
    S: AsyncCatalogStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncCatalogStore::list_hosts(
            store,
            tenant_id,
            Page {
                limit: HOST_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let batch_len = batch.len();
        items.extend(batch);
        if batch_len < HOST_PAGE_LIMIT as usize {
            break;
        }
        offset += batch_len as u32;
    }
    Ok(items)
}
