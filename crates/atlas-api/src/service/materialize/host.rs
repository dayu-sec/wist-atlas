use std::collections::BTreeSet;

use atlas_domain::{
    BindingScope, BusinessCatalogCandidate, Confidence, HostCandidate, HostInventory, HostNetAssoc,
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
use crate::pipeline::{
    InMemoryCatalog, host_net_assoc_for, materialize_host_network, resolve_host_candidate,
    resolve_network_candidate, upsert_segment,
};

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

/// 写「主机 ↔ 网段」关联，并把同一台主机其它仍然敞开的关联封口。
///
/// 为什么必须封口：关联 ID 里含 IP（见 `pipeline::host_net_assoc_for`），主机换私网 IP
/// 就是一条新行。不关旧的，一台机在拓扑里会同时挂在两个子网、两个 IP 上，而且旧的那条
/// 永远显示「现在有效」—— 这比丢数据更糟，因为它看起来是对的。
fn upsert_host_net_assoc<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    assoc: &HostNetAssoc,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    for mut existing in open_assocs_to_close(catalog, assoc) {
        existing.validity.valid_to = Some(now);
        existing.updated_at = now;
        store.upsert_host_net_assoc(&existing)?;
        replace_catalog_assoc(catalog, existing);
    }

    store.upsert_host_net_assoc(assoc)?;
    replace_catalog_assoc(catalog, assoc.clone());
    Ok(())
}

async fn upsert_host_net_assoc_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    assoc: &HostNetAssoc,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    for mut existing in open_assocs_to_close(catalog, assoc) {
        existing.validity.valid_to = Some(now);
        existing.updated_at = now;
        atlas_storage::AsyncRuntimeStore::upsert_host_net_assoc(store, &existing).await?;
        replace_catalog_assoc(catalog, existing);
    }

    atlas_storage::AsyncRuntimeStore::upsert_host_net_assoc(store, assoc).await?;
    replace_catalog_assoc(catalog, assoc.clone());
    Ok(())
}

/// 同一台主机上、与这条不同的、还没封口的关联。
fn open_assocs_to_close(catalog: &InMemoryCatalog, assoc: &HostNetAssoc) -> Vec<HostNetAssoc> {
    catalog
        .host_net_assocs
        .iter()
        .filter(|existing| {
            existing.host_id == assoc.host_id
                && existing.assoc_id != assoc.assoc_id
                && existing.validity.valid_to.is_none()
        })
        .cloned()
        .collect()
}

fn replace_catalog_assoc(catalog: &mut InMemoryCatalog, assoc: HostNetAssoc) {
    if let Some(existing) = catalog
        .host_net_assocs
        .iter_mut()
        .find(|item| item.assoc_id == assoc.assoc_id)
    {
        *existing = assoc;
    } else {
        catalog.host_net_assocs.push(assoc);
    }
}

/// 网络域（云侧 VPC）落地。**必须在网段之前**：段靠域的稳定 ID 挂进去。
pub(crate) fn materialize_network_domains<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    candidates: Vec<atlas_domain::NetworkDomainCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<usize>
where
    S: CatalogStore,
{
    let count = candidates.len();
    for candidate in candidates {
        crate::pipeline::ensure_cloud_network_domain(
            catalog,
            candidate.tenant_id,
            &candidate.external_ref,
            candidate.name.as_deref(),
            candidate.cidr.as_deref(),
            now,
        );
    }
    for domain in &catalog.network_domains {
        store.upsert_network_domain(domain)?;
    }
    Ok(count)
}

pub(crate) async fn materialize_network_domains_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    candidates: Vec<atlas_domain::NetworkDomainCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<usize>
where
    S: AsyncCatalogStore,
{
    let count = candidates.len();
    for candidate in candidates {
        crate::pipeline::ensure_cloud_network_domain(
            catalog,
            candidate.tenant_id,
            &candidate.external_ref,
            candidate.name.as_deref(),
            candidate.cidr.as_deref(),
            now,
        );
    }
    for domain in &catalog.network_domains {
        atlas_storage::AsyncCatalogStore::upsert_network_domain(store, domain).await?;
    }
    Ok(count)
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
        // 段与域**独立落地**：云侧的 VPC / 子网没有 machine_id / host_name，
        // 解析不到主机。以前这种情况整段被跳过，等于云上网络结构一行都没入库。
        let network_resolution = resolve_network_candidate(catalog, &network_candidate, now);
        upsert_segment(catalog, network_resolution.segment.clone());
        for domain in &catalog.network_domains {
            store.upsert_network_domain(domain)?;
        }
        store.upsert_network_segment(&network_resolution.segment)?;
        result.network_count += 1;

        // 有地址、且能对上主机，才建「主机 ↔ 网段」关联。
        if network_candidate.ip_addr.is_none() {
            continue;
        }
        let Some(host_candidate) =
            resolve_host_candidate_for_network(&hosts, catalog, &network_candidate)
        else {
            continue;
        };

        let materialized =
            materialize_host_network(catalog, &host_candidate, &network_candidate, now);
        let mut host = materialized.host;
        mark_seen(&mut host, &host_candidate, scope, now);
        upsert_catalog_host(catalog, host.clone());
        store.upsert_host(&host)?;
        store.upsert_network_segment(&materialized.segment)?;
        if let Some(assoc) = &materialized.assoc {
            upsert_host_net_assoc(store, catalog, assoc, now)?;
            result.assoc_count += 1;
        }
        result.host_ids.insert(host.host_id);
        result.host_count += 1;
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
        // 云侧主机带了私网 IP：把「它在哪个子网」建成关联。
        // 优先用 SubnetId，没有就按 CIDR 包含（最长前缀）兜底。
        if let Some(assoc) = host_net_assoc_for(catalog, &host_candidate, host.host_id, now) {
            upsert_host_net_assoc(store, catalog, &assoc, now)?;
            result.assoc_count += 1;
        }
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
        let network_resolution = resolve_network_candidate(catalog, &network_candidate, now);
        upsert_segment(catalog, network_resolution.segment.clone());
        for domain in &catalog.network_domains {
            atlas_storage::AsyncCatalogStore::upsert_network_domain(store, domain).await?;
        }
        atlas_storage::AsyncCatalogStore::upsert_network_segment(
            store,
            &network_resolution.segment,
        )
        .await?;
        result.network_count += 1;

        if network_candidate.ip_addr.is_none() {
            continue;
        }
        let Some(host_candidate) =
            resolve_host_candidate_for_network(&hosts, catalog, &network_candidate)
        else {
            continue;
        };

        let materialized =
            materialize_host_network(catalog, &host_candidate, &network_candidate, now);
        let mut host = materialized.host;
        mark_seen(&mut host, &host_candidate, scope, now);
        upsert_catalog_host(catalog, host.clone());
        atlas_storage::AsyncCatalogStore::upsert_host(store, &host).await?;
        if let Some(assoc) = &materialized.assoc {
            upsert_host_net_assoc_async(store, catalog, assoc, now).await?;
            result.assoc_count += 1;
        }
        result.host_ids.insert(host.host_id);
        result.host_count += 1;
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
        if let Some(assoc) = host_net_assoc_for(catalog, &host_candidate, host.host_id, now) {
            upsert_host_net_assoc_async(store, catalog, &assoc, now).await?;
            result.assoc_count += 1;
        }
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
                cloud_account: host.cloud_account.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
                ip_addr: None,
                subnet_external_ref: None,
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
                cloud_account: host.cloud_account.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
                ip_addr: None,
                subnet_external_ref: None,
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
                cloud_account: host.cloud_account.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
                ip_addr: None,
                subnet_external_ref: None,
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
                cloud_account: host.cloud_account.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
                ip_addr: None,
                subnet_external_ref: None,
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
    // 账号归属由云侧独占写入；端侧候选通常把它留空，所以只在前者给值时才动。
    // 用「有值就覆盖」而不是「空才填」：采集器改了账号键（source_code → AccountId）
    // 时，存量行要能自我修正，否则它们永远匹配不上退役判定。
    if candidate.cloud_account.is_some() {
        host.cloud_account = candidate.cloud_account.clone();
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
/// 3. **只动本账号的行**：目标集是 `cloud_account == account` 且带 `cloud_instance_id`
///    的主机。**不能用 `ingest_scope`**：那是「最后谁写过」，一台装了 agentd 的云主机
///    每 5 分钟就被端侧覆盖一次，云侧那轮永远扫不到它。`cloud_account` 是资产属性，
///    不随采集顺序变化；IDC 机器没有它，云侧永远不碰。
pub(crate) fn mark_stale_hosts<S>(
    store: &S,
    tenant_id: TenantId,
    seen: &BTreeSet<Uuid>,
    account: &str,
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
        // 本账号的云资产才算本次快照的覆盖对象；端侧发现的主机（没有 cloud_account）不参与。
        if host.cloud_account.as_deref() != Some(account) || host.cloud_instance_id.is_none() {
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
    account: &str,
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
        // 本账号的云资产才算本次快照的覆盖对象；端侧发现的主机（没有 cloud_account）不参与。
        if host.cloud_account.as_deref() != Some(account) || host.cloud_instance_id.is_none() {
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
