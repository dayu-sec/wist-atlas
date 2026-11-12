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
            mark_seen(&mut host, &host_candidate, scope, now);
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
        mark_seen(&mut host, &host_candidate, scope, now);
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
            mark_seen(&mut host, &host_candidate, scope, now);
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
        mark_seen(&mut host, &host_candidate, scope, now);
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
                cloud_instance_id: host.cloud_instance_id.clone(),
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
                cloud_instance_id: host.cloud_instance_id.clone(),
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
                cloud_instance_id: host.cloud_instance_id.clone(),
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
                cloud_instance_id: host.cloud_instance_id.clone(),
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
/// 六件事：**刷新描述字段**、刷新 `last_inventory_at`、作用域归属、缺失计数归零、
/// 清掉 stale —— 也就是「重新出现即复活」。
///
/// 为什么必须显式刷新：`resolve_host_candidate` 匹配到已有主机时是**克隆旧行**的，
/// 不刷新的话一条主机记录的 `os_name` / `arch` 会永远停在它**第一次**被看到时的值 ——
/// 升级 agentd 补了新属性也永远进不来（第一次看到它的往往是老版本）。
///
/// `last_inventory_at` 同理：它是「可达性」的唯一事实来源，只建行时写一次的话，
/// 一台天天在上报的机器会永远显示「最后采集于首次发现时间」。
///
/// 只覆盖**候选给了值**的字段：老 agentd 不报 `os.*` / `arch`，用 `None` 擦掉已知值
/// 比不刷新更糟（会把一次降级安装变成一次数据丢失）。
///
/// `stale_at` 只由退役清扫写（云端确认销毁才写），这里一律写 `None`。
fn mark_seen(host: &mut HostInventory, candidate: &HostCandidate, scope: &str, now: DateTime<Utc>) {
    if candidate.os_name.is_some() {
        host.os_name = candidate.os_name.clone();
    }
    if candidate.os_version.is_some() {
        host.os_version = candidate.os_version.clone();
    }
    if candidate.arch.is_some() {
        host.arch = candidate.arch.clone();
    }
    host.last_inventory_at = now;
    host.stale_at = None;
    host.miss_count = 0;
    host.ingest_scope = Some(scope.to_string());
}

/// 连续多少份**声明账号级完整**的快照没看到一台主机之后，判定它已在云端销毁。
///
/// 为什么是「完整的快照」而不是「没看到它」：云 API 少一页、一个地域超时、限流、
/// 权限不足，在数据上跟「实例被销毁」长得一模一样。不完整的那一轮根本不是一份快照，
/// 既不加计数也不清零。
///
/// 为什么是 3 轮：一个账号级快照的成本很高（全地域全类型），失败一轮很常见；
/// 误删一台活主机的代价远大于晚几轮才标记。
pub(crate) const HOST_RETIRE_AFTER_COMPLETE_SNAPSHOTS: i64 = 3;

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

/// 主机退役清扫：本份快照没出现的主机累加缺失计数，达到阈值才标 `stale_at`。
///
/// **调用方必须先把关**：只有信封声明了 `coverage = account + complete` 才能调它
/// （见 `IngestEnvelope::declares_full_account_coverage`）。这个函数本身分不清
/// 「这一帧没采全」和「这些机器真没了」。
///
/// 与进程清扫的三点不同（都不是可选项）：
///
/// 1. **按阈值、不按单次**。见 [`HOST_RETIRE_AFTER_COMPLETE_SNAPSHOTS`]。
/// 2. **已退役的冻结**：`stale_at` 保留首次判定时间，也不再继续累加计数。
/// 3. **只动本作用域的行**：不许拿云账号的快照去扫端侧发现的主机。代价是
///    「被 agentd 最后写过的那台云主机」暂时扫不到（`ingest_scope` 是最后写入者），
///    这是**故意选的方向**：少退役一台（安全）比跨账号误删一台强。
///    按云账号圈定目标的那一步（`cloud_account`）是后续改动。
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
        if host.miss_count >= HOST_RETIRE_AFTER_COMPLETE_SNAPSHOTS {
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
        if host.miss_count >= HOST_RETIRE_AFTER_COMPLETE_SNAPSHOTS {
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
