use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{EnvironmentId, TenantId, ValidityWindow};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServiceBoundary {
    Internal,
    External,
    Partner,
    Saas,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServiceType {
    Application,
    Data,
    Platform,
    Shared,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkloadKind {
    Deployment,
    StatefulSet,
    DaemonSet,
    Job,
    CronJob,
    BareProcess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubjectType {
    User,
    Team,
    Rotation,
    ServiceAccount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkDomainKind {
    Lan,
    Wan,
    Vpc,
    Vnet,
    Vlan,
    Overlay,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessDomain {
    pub business_id: Uuid,
    pub tenant_id: TenantId,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemBoundary {
    pub system_id: Uuid,
    pub tenant_id: TenantId,
    pub business_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subsystem {
    pub subsystem_id: Uuid,
    pub tenant_id: TenantId,
    pub system_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceEntity {
    pub service_id: Uuid,
    pub tenant_id: TenantId,
    pub business_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub subsystem_id: Option<Uuid>,
    pub name: String,
    pub namespace: Option<String>,
    pub service_type: ServiceType,
    pub boundary: ServiceBoundary,
    pub provider: Option<String>,
    pub external_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClusterInventory {
    pub cluster_id: Uuid,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub name: String,
    pub provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamespaceInventory {
    pub namespace_id: Uuid,
    pub tenant_id: TenantId,
    pub cluster_id: Uuid,
    pub name: String,
    pub environment_id: Option<EnvironmentId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadEntity {
    pub workload_id: Uuid,
    pub tenant_id: TenantId,
    pub cluster_id: Uuid,
    pub namespace_id: Uuid,
    pub service_id: Option<Uuid>,
    pub kind: WorkloadKind,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PodInventory {
    pub pod_id: Uuid,
    pub tenant_id: TenantId,
    pub cluster_id: Uuid,
    pub namespace_id: Uuid,
    pub workload_id: Option<Uuid>,
    pub pod_uid: String,
    pub pod_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostInventory {
    pub host_id: Uuid,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub host_name: String,
    pub machine_id: Option<String>,
    /// 云实例 ID（`InstanceId`）：**跨来源归并键**。
    ///
    /// agentd 从云元数据服务读到自己的 `instance-id`，云 API（wist-harvest）直接给
    /// `InstanceId`。两者相等即同一台机器 —— 这是双来源合并里唯一确定性的证据。
    ///
    /// 与 `machine_id` 分开存：`machine_id` 是**机器自证**（`/etc/machine-id`），
    /// 一台云主机两个都有，用哪个都能对上；混成一个字段会让"云上改了实例"与
    /// "机器重装系统"看起来是同一件事。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_instance_id: Option<String>,
    /// 云端权威来源对该资产的**账号归属**（`vendor:account`，过渡期用采集器声明的账号键）。
    ///
    /// 这是退役判定的作用域：只有本账号的完整快照里没它，才说明它在这个账号下不存在。
    ///
    /// 与 `ingest_scope` 的区别：后者是「最后谁写过」，会随 agentd / 云侧来回翻转；
    /// 这个是**资产属性**，不随采集顺序变化 —— 一台装了 agent 的云主机靠它才能被云侧判死。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_account: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    /// CPU 架构（`aarch64` / `x86_64`）。与事实摘要的 `arch` 同一取值（`std::env::consts::ARCH`）。
    ///
    /// 包/二进制匹配、漏洞影响面都靠它：只知道「一台 macOS 26.6.2 的机器」分不出
    /// 该发 arm64 还是 x86_64 的东西。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    pub created_at: DateTime<Utc>,
    /// 最近一次被**任一来源**看到的时刻。每次出现都刷新。
    ///
    /// 这是「可达性」的唯一事实来源。注意它**不表达存在性**：这个值停在三天前
    /// 只说明三天没有观测，机器可能是关机、agent 停了、网不通 —— 三者在这里是同一件事。
    pub last_inventory_at: DateTime<Utc>,
    /// 首次判定**退役**（云上确认销毁）的时刻；`None` = 仍存在。
    ///
    /// 与进程的 stale **不是同一个判据**，而且比它严得多：进程是「某来源报的这份列表里
    /// 没有它」，主机是「账号级完整快照连续多轮都没有它」。**端侧缺席永远不写这个字段** ——
    /// 采集不到只说明观测不到，不说明机器不存在。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_at: Option<DateTime<Utc>>,
    /// 连续多少份**声明账号级完整**的快照里没出现它。出现即清零；达到阈值才写 `stale_at`。
    ///
    /// 不完整的轮次既不加也不清零：它不被当作一份快照。
    #[serde(default)]
    pub miss_count: i64,
    /// 归属的快照作用域 `schema:system:producer:tenant:env`（见 `ProcessRuntimeState::ingest_scope`）。
    ///
    /// 主机退役**已经不看它了**（那是「最后谁写过」，会被 agentd 每 5 分钟覆盖一次，
    /// 真正的判据是 `cloud_account`）。保留它只为诊断：能回答「这一行最后一次是谁写的」。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingest_scope: Option<String>,
}

/// 主机「最近还有没有被任何来源看到」的派生状态。
///
/// **与存在性正交**：关机、agent 停止上报、网络不通在 atlas 侧是同一件事 —— 没有新观测。
/// 三者都不说明机器不存在，所以 `Silent` 不能让主机从资产目录里消失，只能让界面显示
/// 「最近没有上报」。要判断它还在不在，得看权威来源（云账号完整快照）有没有它。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationState {
    /// 最近一个上报周期内见过。
    Live,
    /// 超过一个上报周期，但还在一天之内。
    Quiet,
    /// 超过一天没有新观测。**不代表不存在**，只是没有证据说明它还在。
    Silent,
}

impl ObservationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Quiet => "quiet",
            Self::Silent => "silent",
        }
    }
}

/// `Live` 的窗口：要容得下最快来源的一个上报周期（agentd 默认 5 分钟）。
///
/// 注意：只由云 API 采集的主机（没有装 agent）每 6 小时才刷新一次 `last_inventory_at`，
/// 所以它们几乎总会落在 `Quiet` —— 这是**预期**，不是告警。要判断这类机器在不在，
/// 看的是云侧完整快照有没有它（见 `stale_at`），而不是可达性标签。
pub const OBSERVATION_LIVE_SECONDS: i64 = 10 * 60;
/// `Quiet` 的窗口：一天。超过它就只剩历史观测，不能再声称「现在还看得到」。
pub const OBSERVATION_QUIET_SECONDS: i64 = 24 * 60 * 60;

impl HostInventory {
    /// 按 `last_inventory_at` 算可达性。未来时间戳（时钟偏差）按 `Live` 处理 ——
    /// 宁可说「刚看到」也不要因为时钟把一台正常上报的机器标成 `Silent`。
    pub fn observation_state(&self, now: DateTime<Utc>) -> ObservationState {
        let age = (now - self.last_inventory_at).num_seconds();
        if age <= OBSERVATION_LIVE_SECONDS {
            ObservationState::Live
        } else if age <= OBSERVATION_QUIET_SECONDS {
            ObservationState::Quiet
        } else {
            ObservationState::Silent
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkDomain {
    pub network_domain_id: Uuid,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    /// 云侧 ID（`VpcId`）。**名字不是身份**：腾讯云每个地域都送建 `Default-VPC`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ref: Option<String>,
    pub name: String,
    pub kind: NetworkDomainKind,
    /// 地址空间（`10.0.0.0/16`）。VPC 的核心属性：不做目录就回答不了「谁和谁同一个地址空间」。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cidr: Option<String>,
    pub description: Option<String>,
    /// 云端账号归属（退役判定的作用域）。端侧来源没有它。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_account: Option<String>,
    /// 首次判定云上已删除的时刻。**缺席永不写它**。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub miss_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingest_scope: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkSegment {
    pub network_segment_id: Uuid,
    pub tenant_id: TenantId,
    pub network_domain_id: Option<Uuid>,
    pub environment_id: Option<EnvironmentId>,
    /// 云侧 ID（`SubnetId`）。子网名在不同 VPC 之间可以重复。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ref: Option<String>,
    pub name: String,
    pub cidr: Option<String>,
    pub gateway_ip: Option<String>,
    pub address_family: AddressFamily,
    /// 云端账号归属（退役判定的作用域）。端侧来源没有它。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_account: Option<String>,
    /// 首次判定云上已删除的时刻。**缺席永不写它**。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub miss_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingest_scope: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostNetAssoc {
    pub assoc_id: Uuid,
    pub tenant_id: TenantId,
    pub host_id: Uuid,
    pub network_segment_id: Uuid,
    pub ip_addr: String,
    pub iface_name: Option<String>,
    pub validity: ValidityWindow,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 弹性公网 IP（Elastic IP）。
///
/// **它是一类资产，不是主机的一个字段**：云上可以先申请、后绑定、解绑后仍保留，
/// 甚至换一台机器绑同一个地址；反过来机器下线了 EIP 还在。所以它有独立的生命周期、
/// 独立的计费（闲置也计费）和独立的归集（属于账号/项目，不属于某台机）。
///
/// 与实例自带公网 IP 的区别：后者没有独立地址 ID，跟实例同生共死，不在这个对象里 ——
/// 那是主机侧的观测属性（`CLOUD_HOST.public_ips`）。把两者摞成一个"公网地址"类型会让
/// 生命周期语义失效（而生命周期正是成本与暴露面分析要的东西）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElasticIp {
    pub elastic_ip_id: Uuid,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    /// 云侧地址 ID（腾讯云 `AddressId`）：重新绑定/换机器都不变，是稳定键。
    pub external_ref: String,
    /// 地址本身（IPv4 字符串）。
    pub address: String,
    pub status: ElasticIpStatus,
    /// 计费方式（`TRAFFIC_POSTPAID_BY_HOUR` 等）。闲置也计费，成本域要用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charge_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bandwidth_mbps: Option<i64>,
    /// 云端账号归属（与主机同一个键）：退役判定的作用域。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_account: Option<String>,
    /// 当前绑定（没绑 = `None`）。
    ///
    /// 只存**当前**绑定：主机下线了 EIP 还在（所以要独立对象），但“上个月绑在谁身上”
    /// 这类历史要单独建绑定表才能回答 —— 现在没消费方，不提前拆。
    /// 真要做时：`ElasticIpBinding` 加 `valid_from/valid_to`，这张表变“当前态”。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<ElasticIpBinding>,
    pub created_at: DateTime<Utc>,
    /// 最近一次被看到的时刻（可达性，与存在性无关）。
    pub last_inventory_at: DateTime<Utc>,
    /// 首次判定释放（云上确认不存在）的时刻。**端侧缺席永不写它**。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub miss_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingest_scope: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElasticIpStatus {
    /// 已绑定到某个目标（实例 / NAT / 负载均衡）。
    Bound,
    /// 已申请但没绑（照样计费，成本治理最关心这个状态）。
    Unbound,
    Unknown,
}

impl ElasticIpStatus {
    /// 从云侧 `AddressStatus`（`BIND` / `UNBIND` …）归一化。认不出来就 `Unknown`，
    /// 不要猜成 `Unbound` —— 那会让「闲置 IP」这件事凭空多出几个假信号。
    pub fn from_reported(value: &str) -> Self {
        match value.trim().to_ascii_uppercase().as_str() {
            "BIND" | "BOUND" => Self::Bound,
            "UNBIND" | "UNBOUND" => Self::Unbound,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::Unbound => "unbound",
            Self::Unknown => "unknown",
        }
    }
}

/// EIP 当前绑在哪。
///
/// 为什么把目标 ID 也留着：目标不一定是主机（NAT / CLB 还不是本仓的对象），
/// 认不出来时至少能说“它绑在 `nat-xxxx` 上”，而不是丢掉这个事实。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElasticIpBinding {
    pub target_kind: ElasticIpTarget,
    /// 目标是主机、且解析得到时才有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_id: Option<Uuid>,
    /// 云的原始目标 ID（`InstanceId`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_target_ref: Option<String>,
    /// 这个绑定是什么时候开始的（首次看到它绑在这个目标上）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElasticIpTarget {
    Host,
    NatGateway,
    LoadBalancer,
    /// 云侧报了目标但类型认不出来（新资源、或字段缺失）。
    Unknown,
}

impl ElasticIpTarget {
    /// 从云侧 `InstanceType` 归一化（`CVM` / `NAT` / `CLB` …）。
    pub fn from_reported(value: &str) -> Self {
        match value.trim().to_ascii_uppercase().as_str() {
            "CVM" | "HOST" | "INSTANCE" => Self::Host,
            "NAT" | "NAT_GATEWAY" | "NATINSTANCE" => Self::NatGateway,
            "CLB" | "LOADBALANCER" | "LB" => Self::LoadBalancer,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::NatGateway => "nat_gateway",
            Self::LoadBalancer => "load_balancer",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subject {
    pub subject_id: Uuid,
    pub tenant_id: TenantId,
    pub subject_type: SubjectType,
    pub display_name: String,
    pub external_ref: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(seen_ago_seconds: i64) -> (HostInventory, DateTime<Utc>) {
        let now = Utc::now();
        let host = HostInventory {
            host_id: Uuid::new_v4(),
            tenant_id: TenantId(Uuid::new_v4()),
            environment_id: None,
            host_name: "node-1".to_string(),
            machine_id: Some("machine-1".to_string()),
            cloud_instance_id: None,
            cloud_account: None,
            os_name: None,
            os_version: None,
            arch: None,
            created_at: now,
            last_inventory_at: now - chrono::Duration::seconds(seen_ago_seconds),
            stale_at: None,
            miss_count: 0,
            ingest_scope: None,
        };
        (host, now)
    }

    #[test]
    fn observation_state_splits_live_quiet_and_silent() {
        // 「最近有没有观测」是一条独立的轴：silent ≠ 不存在，所以它只影响展示。
        let (live, now) = host(60);
        assert_eq!(live.observation_state(now), ObservationState::Live);

        let (quiet, now) = host(OBSERVATION_LIVE_SECONDS + 60);
        assert_eq!(quiet.observation_state(now), ObservationState::Quiet);

        let (silent, now) = host(OBSERVATION_QUIET_SECONDS + 60);
        assert_eq!(silent.observation_state(now), ObservationState::Silent);
        assert!(silent.stale_at.is_none(), "silent 不写 stale_at");
    }

    #[test]
    fn observation_state_boundaries_are_inclusive() {
        // 边界必须钉住：改成 `age < 600` 之类的差一错误没人看得出来。
        let (exactly_live, now) = host(OBSERVATION_LIVE_SECONDS);
        assert_eq!(exactly_live.observation_state(now), ObservationState::Live);
        let (just_quiet, now) = host(OBSERVATION_LIVE_SECONDS + 1);
        assert_eq!(just_quiet.observation_state(now), ObservationState::Quiet);

        let (exactly_quiet, now) = host(OBSERVATION_QUIET_SECONDS);
        assert_eq!(
            exactly_quiet.observation_state(now),
            ObservationState::Quiet
        );
        let (just_silent, now) = host(OBSERVATION_QUIET_SECONDS + 1);
        assert_eq!(just_silent.observation_state(now), ObservationState::Silent);
    }

    #[test]
    fn observation_state_treats_future_timestamps_as_live() {
        // 时钟偏差下宁可说「刚看到」，也别把正常上报的机器标成 silent。
        let (host, now) = host(-3600);
        assert_eq!(host.observation_state(now), ObservationState::Live);
    }

    #[test]
    fn observation_state_serializes_as_snake_case() {
        assert_eq!(ObservationState::Live.as_str(), "live");
        assert_eq!(
            serde_json::to_value(ObservationState::Silent).unwrap(),
            serde_json::json!("silent")
        );
    }
}
