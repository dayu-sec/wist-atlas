use std::collections::BTreeSet;

use atlas_domain::{AtlasInputEnvelope, IngestEnvelope};
use atlas_storage::AsyncIngestStore;
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    IngestStore, RuntimeStore,
};
use chrono::{DateTime, Utc};
use orion_error::conversion::ConvErr;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiResult;
use crate::ingest::{
    IngestJobRecord, extract_business_catalog_candidates, extract_container_runtime_candidates,
    extract_elastic_ip_candidates, extract_host_candidates, extract_host_telemetry_candidates,
    extract_network_domain_candidates, extract_network_segment_candidates,
    extract_process_runtime_candidates, extract_process_telemetry_candidates,
    extract_responsibility_assignment_candidates, extract_service_endpoint_candidates,
    extract_subject_candidates,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineRunSummary {
    pub ingest_id: String,
    pub accepted_at: DateTime<Utc>,
    pub host_count: usize,
    pub network_count: usize,
    pub assoc_count: usize,
    /// 本次快照替换清扫**新判定退役**的主机数（非 snapshot 模式恒为 0）。
    pub stale_host_count: u64,
    /// 本次快照替换清扫标成 stale 的进程数（非 snapshot 模式恒为 0）。
    pub stale_process_count: u64,
    /// 本次写进去的监听端口数。
    pub service_endpoint_count: usize,
    /// 本次关闭（`valid_to` 置位）的过期端点数。
    pub stale_endpoint_count: u64,
    /// 有监听地址但没入目录的候选数（宿主解析不到 / 找不到持有进程 / 可执行名为空）。
    /// 大于 0 说明采集侧给了端口但 pid 没解析出来，是排障的第一线索。
    pub unresolved_endpoint_count: usize,
    /// 本次写进去的容器数。
    pub container_count: usize,
    /// 本次快照替换清扫标成 stale 的容器数（非 snapshot 模式恒为 0）。
    pub stale_container_count: u64,
    /// 宿主解析不到的容器候选数。大于 0 说明采集侧报了容器但我们不知道怎么归属。
    pub unresolved_container_count: usize,
    /// 本次写进去的弹性公网 IP 数。
    #[serde(default)]
    pub elastic_ip_count: usize,
    /// 本次判定已释放的 EIP 数（非完整快照 / 本帧没观测 EIP 时恒为 0）。
    #[serde(default)]
    pub stale_elastic_ip_count: u64,
    /// 本次判定已删除的子网数（域会跟着「一个活段都不剩」一起退役，不单独计数）。
    #[serde(default)]
    pub stale_network_segment_count: u64,
    /// 本帧本可退役，但该账号的退役权在别的生产者手里（值是那个生产者）。
    ///
    /// 同一账号多个生产者时报出来，运维才能发现「两个采集器抢同一个账号」这件错事。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retirement_held_by: Option<String>,
}

pub struct TopologyIngestService<S> {
    store: S,
}

impl<S> TopologyIngestService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> TopologyIngestService<S>
where
    S: CatalogStore + RuntimeStore + IngestStore + GovernanceStore,
{
    pub fn submit_and_materialize(
        &self,
        envelope: IngestEnvelope,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        let accepted_at = envelope.received_at;
        let record = validate_and_record(&self.store, &envelope)?;

        let hosts = extract_host_candidates(&envelope)?.candidates;
        let network_domains = extract_network_domain_candidates(&envelope)?.candidates;
        let network_extraction = extract_network_segment_candidates(&envelope)?;
        let networks_reported = network_extraction.reported;
        let networks = network_extraction.candidates;
        let elastic_ips = extract_elastic_ip_candidates(&envelope)?;
        let processes = extract_process_runtime_candidates(&envelope)?.candidates;
        let container_extraction = extract_container_runtime_candidates(&envelope)?;
        let endpoint_extraction = extract_service_endpoint_candidates(&envelope)?;
        let business_catalog = extract_business_catalog_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let host_telemetry = extract_host_telemetry_candidates(&envelope)?.candidates;
        let process_telemetry = extract_process_telemetry_candidates(&envelope)?.candidates;
        let subjects = extract_subject_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let assignments = extract_responsibility_assignment_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();

        let mut catalog = hydrate_catalog(&self.store, envelope.tenant_id).conv_err()?;
        let scope = envelope.ingest_scope();
        // 容器 id 是确定性的：在物化进程之前就能算出本轮「确实采到的容器」集合，
        // 进程与端口共用它来判断容器归属，避免写出指向不存在容器的引用。
        let known_containers: BTreeSet<Uuid> = container_extraction
            .candidates
            .iter()
            .map(|candidate| container_uuid(candidate.tenant_id, &candidate.container_key))
            .collect();
        materialize_business_catalog(&self.store, business_catalog, accepted_at).conv_err()?;
        // 退役权每帧只登记一次（三组清扫共用）：同一账号多个生产者时不双倍推进。
        let retirement = retirement_gate(&self.store, &envelope, accepted_at)?;
        // 域必须在段之前：段靠域的稳定 ID 挂进去。
        materialize_network_domains(
            &self.store,
            &mut catalog,
            network_domains,
            &scope,
            accepted_at,
        )
        .conv_err()?;
        let materialized = materialize_candidates(
            &self.store,
            &mut catalog,
            hosts,
            networks,
            accepted_at,
            &scope,
        )
        .conv_err()?;
        let host_stale_count = sweep_stale_hosts(
            &self.store,
            &envelope,
            &materialized,
            &retirement,
            accepted_at,
        )
        .conv_err()?;
        // EIP 必须在主机之后：绑定目标要按刚落地的主机解析。
        let materialized_ips = materialize_elastic_ips(
            &self.store,
            &catalog,
            elastic_ips.candidates,
            accepted_at,
            &scope,
        )
        .conv_err()?;
        let stale_elastic_ip_count = sweep_stale_elastic_ips(
            &self.store,
            &envelope,
            &materialized_ips,
            elastic_ips.reported,
            &retirement,
            accepted_at,
        )
        .conv_err()?;
        let stale_network_count = sweep_stale_networks(
            &self.store,
            &envelope,
            &materialized,
            networks_reported,
            &retirement,
            accepted_at,
        )
        .conv_err()?;
        let process_sweep = materialize_processes(
            &self.store,
            &mut catalog,
            processes,
            &known_containers,
            accepted_at,
            &scope,
        )
        .conv_err()?;
        let stale_process_count =
            sweep_stale_processes(&self.store, &envelope, &process_sweep, &scope, accepted_at)
                .conv_err()?;
        let container_sweep = materialize_containers(
            &self.store,
            &catalog,
            container_extraction.candidates,
            accepted_at,
            &scope,
        )
        .conv_err()?;
        let stale_container_count = if container_extraction.reported {
            mark_stale_containers(&self.store, &container_sweep, &scope, accepted_at).conv_err()?
        } else {
            0
        };
        // 端点必须在进程之后物化：它要按 pid 找到持有 socket 的进程行。
        let endpoint_sweep = materialize_service_endpoints(
            &self.store,
            &catalog,
            endpoint_extraction.candidates,
            endpoint_extraction.reported,
            &known_containers,
            &materialized.host_ids,
            accepted_at,
        )
        .conv_err()?;
        materialize_host_telemetry(&self.store, &mut catalog, host_telemetry).conv_err()?;
        materialize_process_telemetry(&self.store, &mut catalog, process_telemetry).conv_err()?;
        materialize_subjects_and_assignments(
            &self.store,
            envelope.tenant_id,
            subjects,
            assignments,
            accepted_at,
        )
        .conv_err()?;

        Ok((
            record,
            PipelineRunSummary {
                ingest_id: envelope.ingest_id,
                accepted_at,
                host_count: materialized.host_count,
                network_count: materialized.network_count,
                assoc_count: materialized.assoc_count,
                stale_host_count: host_stale_count,
                stale_process_count,
                service_endpoint_count: endpoint_sweep.materialized,
                stale_endpoint_count: endpoint_sweep.closed,
                unresolved_endpoint_count: endpoint_sweep.unresolved,
                container_count: container_sweep.seen.len(),
                stale_container_count,
                unresolved_container_count: container_sweep.unresolved,
                elastic_ip_count: materialized_ips.count,
                stale_elastic_ip_count,
                stale_network_segment_count: stale_network_count,
                retirement_held_by: retirement.held_by.clone(),
            },
        ))
    }

    pub fn submit_atlas_input_and_materialize(
        &self,
        input: AtlasInputEnvelope,
        tenant_id: atlas_domain::TenantId,
        environment_id: Option<atlas_domain::EnvironmentId>,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        input.validate().conv_err()?;
        self.submit_and_materialize(input.into_ingest_envelope(
            tenant_id,
            environment_id,
            Utc::now(),
        ))
    }
}

impl<S> TopologyIngestService<S>
where
    S: AsyncCatalogStore + AsyncRuntimeStore + AsyncIngestStore + AsyncGovernanceStore,
{
    pub async fn submit_and_materialize_async(
        &self,
        envelope: IngestEnvelope,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        let accepted_at = envelope.received_at;
        let record = validate_and_record_async(&self.store, &envelope).await?;

        let hosts = extract_host_candidates(&envelope)?.candidates;
        let network_domains = extract_network_domain_candidates(&envelope)?.candidates;
        let network_extraction = extract_network_segment_candidates(&envelope)?;
        let networks_reported = network_extraction.reported;
        let networks = network_extraction.candidates;
        let elastic_ips = extract_elastic_ip_candidates(&envelope)?;
        let processes = extract_process_runtime_candidates(&envelope)?.candidates;
        let containers = extract_container_runtime_candidates(&envelope)?;
        let endpoints = extract_service_endpoint_candidates(&envelope)?;
        let business_catalog = extract_business_catalog_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let host_telemetry = extract_host_telemetry_candidates(&envelope)?.candidates;
        let process_telemetry = extract_process_telemetry_candidates(&envelope)?.candidates;
        let subjects = extract_subject_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let assignments = extract_responsibility_assignment_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();

        let mut catalog = hydrate_catalog_async(&self.store, envelope.tenant_id)
            .await
            .conv_err()?;
        let scope = envelope.ingest_scope();
        let known_containers: BTreeSet<Uuid> = containers
            .candidates
            .iter()
            .map(|candidate| container_uuid(candidate.tenant_id, &candidate.container_key))
            .collect();
        materialize_business_catalog_async(&self.store, business_catalog, accepted_at)
            .await
            .conv_err()?;
        let retirement = retirement_gate_async(&self.store, &envelope, accepted_at).await?;
        materialize_network_domains_async(
            &self.store,
            &mut catalog,
            network_domains,
            &scope,
            accepted_at,
        )
        .await
        .conv_err()?;
        let materialized = materialize_candidates_async(
            &self.store,
            &mut catalog,
            hosts,
            networks,
            accepted_at,
            &scope,
        )
        .await
        .conv_err()?;
        let host_stale_count = sweep_stale_hosts_async(
            &self.store,
            &envelope,
            &materialized,
            &retirement,
            accepted_at,
        )
        .await
        .conv_err()?;
        let materialized_ips = materialize_elastic_ips_async(
            &self.store,
            &catalog,
            elastic_ips.candidates,
            accepted_at,
            &scope,
        )
        .await
        .conv_err()?;
        let stale_elastic_ip_count = sweep_stale_elastic_ips_async(
            &self.store,
            &envelope,
            &materialized_ips,
            elastic_ips.reported,
            &retirement,
            accepted_at,
        )
        .await
        .conv_err()?;
        let stale_network_count = sweep_stale_networks_async(
            &self.store,
            &envelope,
            &materialized,
            networks_reported,
            &retirement,
            accepted_at,
        )
        .await
        .conv_err()?;
        let process_sweep = materialize_processes_async(
            &self.store,
            &mut catalog,
            processes,
            &known_containers,
            accepted_at,
            &scope,
        )
        .await
        .conv_err()?;
        let stale_process_count = sweep_stale_processes_async(
            &self.store,
            &envelope,
            &process_sweep,
            &scope,
            accepted_at,
        )
        .await
        .conv_err()?;
        let container_sweep = materialize_containers_async(
            &self.store,
            &catalog,
            containers.candidates,
            accepted_at,
            &scope,
        )
        .await
        .conv_err()?;
        let stale_container_count = if containers.reported {
            mark_stale_containers_async(&self.store, &container_sweep, &scope, accepted_at)
                .await
                .conv_err()?
        } else {
            0
        };
        // 端点必须在进程之后物化：它要按 pid 找到持有 socket 的进程行。
        let endpoint_sweep = materialize_service_endpoints_async(
            &self.store,
            &catalog,
            endpoints.candidates,
            endpoints.reported,
            &known_containers,
            &materialized.host_ids,
            accepted_at,
        )
        .await
        .conv_err()?;
        materialize_host_telemetry_async(&self.store, &mut catalog, host_telemetry)
            .await
            .conv_err()?;
        materialize_process_telemetry_async(&self.store, &mut catalog, process_telemetry)
            .await
            .conv_err()?;
        materialize_subjects_and_assignments_async(
            &self.store,
            envelope.tenant_id,
            subjects,
            assignments,
            accepted_at,
        )
        .await
        .conv_err()?;

        Ok((
            record,
            PipelineRunSummary {
                ingest_id: envelope.ingest_id,
                accepted_at,
                host_count: materialized.host_count,
                network_count: materialized.network_count,
                assoc_count: materialized.assoc_count,
                stale_host_count: host_stale_count,
                stale_process_count,
                service_endpoint_count: endpoint_sweep.materialized,
                stale_endpoint_count: endpoint_sweep.closed,
                unresolved_endpoint_count: endpoint_sweep.unresolved,
                container_count: container_sweep.seen.len(),
                stale_container_count,
                unresolved_container_count: container_sweep.unresolved,
                elastic_ip_count: materialized_ips.count,
                stale_elastic_ip_count,
                stale_network_segment_count: stale_network_count,
                retirement_held_by: retirement.held_by.clone(),
            },
        ))
    }

    pub async fn submit_atlas_input_and_materialize_async(
        &self,
        input: AtlasInputEnvelope,
        tenant_id: atlas_domain::TenantId,
        environment_id: Option<atlas_domain::EnvironmentId>,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        input.validate().conv_err()?;
        self.submit_and_materialize_async(input.into_ingest_envelope(
            tenant_id,
            environment_id,
            Utc::now(),
        ))
        .await
    }
}

mod materialize;
use materialize::{
    ContainerSweep, MaterializedElasticIps, MaterializedHosts, ProcessSweep, container_uuid,
    hydrate_catalog, hydrate_catalog_async, mark_stale_containers, mark_stale_containers_async,
    mark_stale_elastic_ips, mark_stale_elastic_ips_async, mark_stale_hosts, mark_stale_hosts_async,
    mark_stale_network_segments, mark_stale_network_segments_async, mark_stale_processes,
    mark_stale_processes_async, materialize_business_catalog, materialize_business_catalog_async,
    materialize_candidates, materialize_candidates_async, materialize_containers,
    materialize_containers_async, materialize_elastic_ips, materialize_elastic_ips_async,
    materialize_host_telemetry, materialize_host_telemetry_async, materialize_network_domains,
    materialize_network_domains_async, materialize_process_telemetry,
    materialize_process_telemetry_async, materialize_processes, materialize_processes_async,
    materialize_service_endpoints, materialize_service_endpoints_async,
    materialize_subjects_and_assignments, materialize_subjects_and_assignments_async,
    validate_and_record, validate_and_record_async,
};

/// 主机退役清扫。
///
/// 两道闸，缺一不可：
///
/// 1. `snapshot` 模式 —— `incremental` / `window` / `correction` 描述的是**部分**观测，
///    拿去当「全集」比对会把没投递的主机累计成退役。
/// 2. **信封声明了账号级完整覆盖** —— 云 API 少一页、地域超时、限流，在数据上跟
///    「实例被销毁」长得一模一样。没声明就当它什么也没说：宁可不退役。
///
/// 端侧（agentd）永远过不了第 2 道闸，这是故意的：采集不到只说明观测不到，
/// 关机 / agent 停掉 / 网络不通在 atlas 侧是同一件事，三者都不说明机器不存在。
fn sweep_stale_hosts<S>(
    store: &S,
    envelope: &IngestEnvelope,
    materialized: &MaterializedHosts,
    gate: &RetirementGate,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: CatalogStore,
{
    let Some(account) = gate.account.as_deref() else {
        return Ok(0);
    };
    Ok(mark_stale_hosts(
        store,
        envelope.tenant_id,
        &materialized.host_ids,
        account,
        at,
    )
    .conv_err()?
    .newly_stale)
}

async fn sweep_stale_hosts_async<S>(
    store: &S,
    envelope: &IngestEnvelope,
    materialized: &MaterializedHosts,
    gate: &RetirementGate,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: AsyncCatalogStore,
{
    let Some(account) = gate.account.as_deref() else {
        return Ok(0);
    };
    Ok(mark_stale_hosts_async(
        store,
        envelope.tenant_id,
        &materialized.host_ids,
        account,
        at,
    )
    .await
    .conv_err()?
    .newly_stale)
}

/// EIP 退役清扫。三道闸缺一不可：
///
/// 1. `snapshot` 模式；
/// 2. 账号级完整覆盖声明（@see `retirement_account`）；
/// 3. **本帧真的观测了 EIP**（`reported`）—— 采集器把 EIP 采集关掉时，
///    「没有它」什么也不证明，否则一次配置变更会把账号下所有 EIP 判死。
fn sweep_stale_elastic_ips<S>(
    store: &S,
    envelope: &IngestEnvelope,
    materialized: &MaterializedElasticIps,
    reported: bool,
    gate: &RetirementGate,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: CatalogStore,
{
    if !reported {
        return Ok(0);
    }
    let Some(account) = gate.account.as_deref() else {
        return Ok(0);
    };
    mark_stale_elastic_ips(
        store,
        envelope.tenant_id,
        &materialized.elastic_ip_ids,
        account,
        at,
    )
    .conv_err()
}

async fn sweep_stale_elastic_ips_async<S>(
    store: &S,
    envelope: &IngestEnvelope,
    materialized: &MaterializedElasticIps,
    reported: bool,
    gate: &RetirementGate,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: AsyncCatalogStore,
{
    if !reported {
        return Ok(0);
    }
    let Some(account) = gate.account.as_deref() else {
        return Ok(0);
    };
    mark_stale_elastic_ips_async(
        store,
        envelope.tenant_id,
        &materialized.elastic_ip_ids,
        account,
        at,
    )
    .await
    .conv_err()
}

/// 子网退役清扫。闸门与主机/EIP 相同，另外要求本帧**观测了网段**
/// （`network_segments` 键在）：采集器不报网络时，「没有它」什么也不证明。
fn sweep_stale_networks<S>(
    store: &S,
    envelope: &IngestEnvelope,
    materialized: &MaterializedHosts,
    reported: bool,
    gate: &RetirementGate,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: CatalogStore,
{
    if !reported {
        return Ok(0);
    }
    let Some(account) = gate.account.as_deref() else {
        return Ok(0);
    };
    mark_stale_network_segments(
        store,
        envelope.tenant_id,
        &materialized.network_segment_ids,
        account,
        at,
    )
    .conv_err()
}

async fn sweep_stale_networks_async<S>(
    store: &S,
    envelope: &IngestEnvelope,
    materialized: &MaterializedHosts,
    reported: bool,
    gate: &RetirementGate,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: AsyncCatalogStore,
{
    if !reported {
        return Ok(0);
    }
    let Some(account) = gate.account.as_deref() else {
        return Ok(0);
    };
    mark_stale_network_segments_async(
        store,
        envelope.tenant_id,
        &materialized.network_segment_ids,
        account,
        at,
    )
    .await
    .conv_err()
}

/// 这一帧有没有资格判定「主机已不存在」？有就返回它覆盖的账号键。
///
/// 两道条件缺一不可：`snapshot` 模式（部分观测不能当全集），以及采集器声明了
/// 账号级完整覆盖（少一页 / 超时 / 限流在数据上跟「销毁」长得一样）。
fn retirement_account(envelope: &IngestEnvelope) -> Option<&str> {
    if envelope.ingest_mode != atlas_domain::IngestMode::Snapshot
        || !envelope.declares_full_account_coverage()
    {
        return None;
    }
    envelope.covered_account()
}

/// 本帧能对哪个账号行使退役权，以及不能时的原因。
#[derive(Debug, Default, Clone)]
struct RetirementGate {
    /// 可以退役的账号键。`None` = 本帧不做退役（不完整 / 不是快照 / 端侧）。
    account: Option<String>,
    /// 本帧本可退役，但账号的退役权在别的生产者手里 —— 值是那个生产者。
    held_by: Option<String>,
}

/// 退役权登记：一个云账号只认一个采集器。
///
/// 为什么需要：两个生产者同时声明同一账号的账号级完整覆盖时，每帧都会给缺行
/// `miss_count + 1`，退役速度直接翻倍（3 轮变 1.5 轮）。方向是错的 —— 采集器抖动
/// 一次就可能把还在的机器判死。
///
/// 处理方式：先到先得，持有者每帧刷新心跳；其他人照常写数据（upsert 幂等），只是不推进
/// 退役。持有者长期不出声（`RETIREMENT_OWNER_TTL_HOURS`）才允许接管，避免换采集器之后
/// 永远冻结。
fn retirement_gate<S>(
    store: &S,
    envelope: &IngestEnvelope,
    at: DateTime<Utc>,
) -> ApiResult<RetirementGate>
where
    S: CatalogStore,
{
    let Some(account) = retirement_account(envelope) else {
        return Ok(RetirementGate::default());
    };
    let producer = envelope.source_name.clone();
    let current = store
        .get_retirement_owner(envelope.tenant_id, account)
        .conv_err()?;

    match current {
        Some(owner) if owner.producer != producer && !owner.is_expired(at) => Ok(RetirementGate {
            account: None,
            held_by: Some(owner.producer),
        }),
        _ => {
            store
                .upsert_retirement_owner(&atlas_domain::RetirementOwner {
                    tenant_id: envelope.tenant_id,
                    cloud_account: account.to_string(),
                    producer,
                    claimed_at: at,
                })
                .conv_err()?;
            Ok(RetirementGate {
                account: Some(account.to_string()),
                held_by: None,
            })
        }
    }
}

async fn retirement_gate_async<S>(
    store: &S,
    envelope: &IngestEnvelope,
    at: DateTime<Utc>,
) -> ApiResult<RetirementGate>
where
    S: AsyncCatalogStore,
{
    let Some(account) = retirement_account(envelope) else {
        return Ok(RetirementGate::default());
    };
    let producer = envelope.source_name.clone();
    let current = store
        .get_retirement_owner(envelope.tenant_id, account)
        .await
        .conv_err()?;

    match current {
        Some(owner) if owner.producer != producer && !owner.is_expired(at) => Ok(RetirementGate {
            account: None,
            held_by: Some(owner.producer),
        }),
        _ => {
            store
                .upsert_retirement_owner(&atlas_domain::RetirementOwner {
                    tenant_id: envelope.tenant_id,
                    cloud_account: account.to_string(),
                    producer,
                    claimed_at: at,
                })
                .await
                .conv_err()?;
            Ok(RetirementGate {
                account: Some(account.to_string()),
                held_by: None,
            })
        }
    }
}

/// 只在 `snapshot` 模式下做替换清扫。
///
/// `incremental` / `window` / `correction` 描述的都是**部分**观测，拿去当「全集」比对会把
/// 没投递的对象全标死。
fn sweep_stale_processes<S>(
    store: &S,
    envelope: &IngestEnvelope,
    sweep: &ProcessSweep,
    scope: &str,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: RuntimeStore,
{
    if envelope.ingest_mode != atlas_domain::IngestMode::Snapshot {
        return Ok(0);
    }
    Ok(mark_stale_processes(store, sweep, scope, at)
        .conv_err()?
        .marked)
}

async fn sweep_stale_processes_async<S>(
    store: &S,
    envelope: &IngestEnvelope,
    sweep: &ProcessSweep,
    scope: &str,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: AsyncRuntimeStore,
{
    if envelope.ingest_mode != atlas_domain::IngestMode::Snapshot {
        return Ok(0);
    }
    Ok(mark_stale_processes_async(store, sweep, scope, at)
        .await
        .conv_err()?
        .marked)
}

#[cfg(test)]
mod tests;
