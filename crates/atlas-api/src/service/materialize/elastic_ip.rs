//! 弹性公网 IP（EIP）的物料化。
//!
//! 三条约束（都来自「EIP 是独立资产」这一点）：
//!
//! 1. **身份是地址 ID**（`AddressId`），不是面积在哪个地址上、也不是绑在哪台机上 ——
//!    地址会换机器、会解绑后保留、解绑后还能重绑到另一台机。
//! 2. **绑定是快照态，不是主机的属性**。目标可能是 CVM / NAT / CLB，识别不出种类时
//!    至少保留原始目标 ID；主机下线了 EIP 还在，所以目标解析失败也不能丢这条 EIP。
//! 3. **退役与主机同一套规则**：只认账号级完整快照，端侧缺席永不判死。
//!    而且只有本帧**真的观测了 EIP**（`reported`）才做比对 —— 采集器关掉 EIP 时
//!    「没有它」什么也不证明。

use std::collections::{BTreeMap, BTreeSet};

use atlas_domain::{
    ElasticIp, ElasticIpBinding, ElasticIpCandidate, ElasticIpStatus, ElasticIpTarget, TenantId,
};
use atlas_storage::{AsyncCatalogStore, CatalogStore, Page, StorageResult};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::stable_uuid;
use crate::pipeline::InMemoryCatalog;

/// 连续多少份**声明账号级完整**的快照里都没有这个地址，才判定它已被释放。
/// 与主机同一个阈值，理由也同一个：误释放一台活资产的代价远大于晚几轮。
pub(crate) const ELASTIC_IP_RETIRE_AFTER_COMPLETE_SNAPSHOTS: i64 = 3;

#[derive(Debug, Default, Clone)]
pub(crate) struct MaterializedElasticIps {
    /// 本帧覆盖到的地址 ID 集（退役比对用）。
    pub elastic_ip_ids: BTreeSet<Uuid>,
    pub count: usize,
}

fn elastic_ip_uuid(tenant_id: TenantId, external_ref: &str) -> Uuid {
    stable_uuid("elastic_ip", &format!("{}:{external_ref}", tenant_id.0))
}

fn full_page() -> Page {
    Page {
        limit: i32::MAX as u32,
        offset: 0,
    }
}

/// 从主机目录里建 `cloud_instance_id → host_id` 索引（绑定目标解析用）。
fn host_index(catalog: &InMemoryCatalog) -> BTreeMap<String, Uuid> {
    catalog
        .hosts
        .iter()
        .filter_map(|host| {
            host.cloud_instance_id
                .as_ref()
                .map(|instance_id| (instance_id.clone(), host.host_id))
        })
        .collect()
}

/// 算「现在的绑定」。目标没变就沿用原来的 `bound_since`。
fn resolve_binding(
    candidate: &ElasticIpCandidate,
    existing: Option<&ElasticIp>,
    hosts: &BTreeMap<String, Uuid>,
    now: DateTime<Utc>,
) -> Option<ElasticIpBinding> {
    let target_ref = candidate.bound_instance_ref.as_deref()?.trim();
    if target_ref.is_empty() {
        return None;
    }

    let bound_since = existing
        .and_then(|ip| ip.binding.as_ref())
        .filter(|binding| binding.external_target_ref.as_deref() == Some(target_ref))
        .and_then(|binding| binding.bound_since)
        .or(Some(now));

    Some(ElasticIpBinding {
        target_kind: ElasticIpTarget::from_reported(
            candidate.bound_instance_type.as_deref().unwrap_or_default(),
        ),
        host_id: hosts.get(target_ref).copied(),
        external_target_ref: Some(target_ref.to_string()),
        bound_since,
    })
}

fn build_elastic_ip(
    candidate: &ElasticIpCandidate,
    existing: Option<&ElasticIp>,
    hosts: &BTreeMap<String, Uuid>,
    now: DateTime<Utc>,
    scope: &str,
) -> ElasticIp {
    let binding = resolve_binding(candidate, existing, hosts, now);
    // 云侧没给状态、但给了绑定目标：按「已绑定」算。
    // 反过来（给了 UNBIND 但残留 InstanceId）以状态为准 —— 那才是云在说的事实。
    let mut status = candidate
        .status
        .as_deref()
        .map(ElasticIpStatus::from_reported)
        .unwrap_or(ElasticIpStatus::Unknown);
    if status == ElasticIpStatus::Unknown && binding.is_some() {
        status = ElasticIpStatus::Bound;
    }

    ElasticIp {
        elastic_ip_id: elastic_ip_uuid(candidate.tenant_id, &candidate.external_ref),
        tenant_id: candidate.tenant_id,
        environment_id: candidate.environment_id,
        external_ref: candidate.external_ref.clone(),
        address: candidate.address.clone(),
        status,
        // 云侧这次没报的字段沿用旧值：与 SQL 里的 COALESCE 同一语义，
        // 也让内存实现与真库行为一致。
        charge_type: candidate
            .charge_type
            .clone()
            .or_else(|| existing.and_then(|ip| ip.charge_type.clone())),
        bandwidth_mbps: candidate
            .bandwidth_mbps
            .or_else(|| existing.and_then(|ip| ip.bandwidth_mbps)),
        cloud_account: candidate
            .cloud_account
            .clone()
            .or_else(|| existing.and_then(|ip| ip.cloud_account.clone())),
        binding,
        created_at: existing.map(|ip| ip.created_at).unwrap_or(now),
        last_inventory_at: now,
        stale_at: None,
        miss_count: 0,
        ingest_scope: Some(scope.to_string()),
    }
}

pub(crate) fn materialize_elastic_ips<S>(
    store: &S,
    catalog: &InMemoryCatalog,
    candidates: Vec<ElasticIpCandidate>,
    now: DateTime<Utc>,
    scope: &str,
) -> StorageResult<MaterializedElasticIps>
where
    S: CatalogStore,
{
    let Some(tenant_id) = candidates.first().map(|candidate| candidate.tenant_id) else {
        return Ok(MaterializedElasticIps::default());
    };

    let existing: BTreeMap<Uuid, ElasticIp> = store
        .list_elastic_ips(tenant_id, full_page())?
        .into_iter()
        .map(|ip| (ip.elastic_ip_id, ip))
        .collect();
    let hosts = host_index(catalog);

    let mut result = MaterializedElasticIps::default();
    for candidate in candidates {
        let id = elastic_ip_uuid(candidate.tenant_id, &candidate.external_ref);
        let ip = build_elastic_ip(&candidate, existing.get(&id), &hosts, now, scope);
        store.upsert_elastic_ip(&ip)?;
        result.elastic_ip_ids.insert(id);
        result.count += 1;
    }

    Ok(result)
}

pub(crate) async fn materialize_elastic_ips_async<S>(
    store: &S,
    catalog: &InMemoryCatalog,
    candidates: Vec<ElasticIpCandidate>,
    now: DateTime<Utc>,
    scope: &str,
) -> StorageResult<MaterializedElasticIps>
where
    S: AsyncCatalogStore,
{
    let Some(tenant_id) = candidates.first().map(|candidate| candidate.tenant_id) else {
        return Ok(MaterializedElasticIps::default());
    };

    let existing: BTreeMap<Uuid, ElasticIp> = store
        .list_elastic_ips(tenant_id, full_page())
        .await?
        .into_iter()
        .map(|ip| (ip.elastic_ip_id, ip))
        .collect();
    let hosts = host_index(catalog);

    let mut result = MaterializedElasticIps::default();
    for candidate in candidates {
        let id = elastic_ip_uuid(candidate.tenant_id, &candidate.external_ref);
        let ip = build_elastic_ip(&candidate, existing.get(&id), &hosts, now, scope);
        store.upsert_elastic_ip(&ip).await?;
        result.elastic_ip_ids.insert(id);
        result.count += 1;
    }

    Ok(result)
}

/// EIP 退役清扫：判据与主机完全一致（本账号 + 完整快照 + 连续 N 轮）。
///
/// **调用方必须先确认本帧真的观测了 EIP**（`ExtractedElasticIps::reported`），
/// 否则「采集器把 EIP 关了」会被当成「所有 EIP 都没了」。
pub(crate) fn mark_stale_elastic_ips<S>(
    store: &S,
    tenant_id: TenantId,
    seen: &BTreeSet<Uuid>,
    account: &str,
    at: DateTime<Utc>,
) -> StorageResult<u64>
where
    S: CatalogStore,
{
    if seen.is_empty() {
        return Ok(0);
    }

    let mut newly_stale = 0;
    for mut ip in store.list_elastic_ips(tenant_id, full_page())? {
        if ip.cloud_account.as_deref() != Some(account) {
            continue;
        }
        if seen.contains(&ip.elastic_ip_id) || ip.stale_at.is_some() {
            continue;
        }

        ip.miss_count += 1;
        if ip.miss_count >= ELASTIC_IP_RETIRE_AFTER_COMPLETE_SNAPSHOTS {
            ip.stale_at = Some(at);
            newly_stale += 1;
        }
        store.upsert_elastic_ip(&ip)?;
    }

    Ok(newly_stale)
}

pub(crate) async fn mark_stale_elastic_ips_async<S>(
    store: &S,
    tenant_id: TenantId,
    seen: &BTreeSet<Uuid>,
    account: &str,
    at: DateTime<Utc>,
) -> StorageResult<u64>
where
    S: AsyncCatalogStore,
{
    if seen.is_empty() {
        return Ok(0);
    }

    let mut newly_stale = 0;
    for mut ip in store.list_elastic_ips(tenant_id, full_page()).await? {
        if ip.cloud_account.as_deref() != Some(account) {
            continue;
        }
        if seen.contains(&ip.elastic_ip_id) || ip.stale_at.is_some() {
            continue;
        }

        ip.miss_count += 1;
        if ip.miss_count >= ELASTIC_IP_RETIRE_AFTER_COMPLETE_SNAPSHOTS {
            ip.stale_at = Some(at);
            newly_stale += 1;
        }
        store.upsert_elastic_ip(&ip).await?;
    }

    Ok(newly_stale)
}
