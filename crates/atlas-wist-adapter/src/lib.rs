//! `wist-agentd` 发现快照 → `atlas.in.edge.v1` 信封。
//!
//! 两个输入口：
//!
//! 1. [`envelope_from_report_json`]：读一帧 `DSNAP:` 的正文（`ReportDiscoverySnapshot` 对象）。
//!    线上路径 —— 网关的 `agent-discovery` sink 落的就是这玩意的字符串形态。
//! 2. [`envelope_from_state_dir`]：直接读 agentd 配置目录下的
//!    `state/discovery/{resources,targets,meta}.json`。离线 P0 路径，不依赖 agentd 发包。
//!
//! # 为什么不用 `wist-contracts` 的类型直接反序列化
//!
//! `wist-contracts` 的 `DiscoverySnapshot` 带 `#[serde(deny_unknown_fields)]`。这对**生产端**
//! 是对的（两侧共用的字节必须严格），对**消费端**是反的：新版 agentd 加一个字段，旧版 atlas
//! 的适配器就会把**整帧**打回，资产目录直接停更 —— 而没被消费的字段本来也不该影响结果。
//!
//! 所以这里用本地容错结构体：只声明我们真正读的字段，其余忽略。
//! 契约改了名字也不会“静默产出空结果”——每个字段都有 fixture 测试钉住。
//!
//! # 身份的两条线
//!
//! `host.id` 在 agentd 读不到 `/etc/machine-id` 时退化成 `hostname:<name>`。这种值**不是**
//! 机器强标识，只进 `external_ref`；当 `machine_id` 用会让同名不同机在中心侧合并成一台。

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 输出信封的 schema 标识。
pub const EDGE_SCHEMA: &str = "atlas.in.edge.v1";

const HOST_KIND: &str = "host";
const IFACE_KIND: &str = "network_interface";
const ADDR_KIND: &str = "ip_address";
const PROCESS_KIND: &str = "process";

/// agentd 没有真实机器标识时的 `host.id` 前缀。
const HOSTNAME_PREFIX: &str = "hostname:";

/// 当前 UTC 时间，RFC3339 微秒精度 + `Z`（与 agentd 的 `now_rfc3339()` 同形）。
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
}

// ── 错误 ────────────────────────────────────────────────────

#[derive(Debug)]
pub enum AdapterError {
    /// 读输入失败。
    Read { path: PathBuf, source: io::Error },
    /// 解析 JSON 失败。
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    /// 快照里没有 host 资源 —— 整份资产没有锚点，继续只会产出一堆无主的进程。
    MissingHost,
    /// report 文档里没有 `snapshot` 对象。
    MissingReportSnapshot,
    /// 两条来源都没有给出 producer（`source.producer` 是必填）。
    MissingProducer,
    /// sink 行里没有可用的 `body`（解析层就没有东西可转，不是“快照不完整”）。
    MissingSinkBody { agent_id: String },
    /// `process.pid` / `net.if.prefix` 不是数字。
    NotNumeric {
        resource_id: String,
        field: &'static str,
        value: String,
    },
    /// 写输出失败。
    Write { path: PathBuf, source: io::Error },
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "read {}: {source}", path.display()),
            Self::Parse { path, source } => write!(f, "parse {}: {source}", path.display()),
            Self::MissingHost => write!(f, "snapshot has no host resource"),
            Self::MissingReportSnapshot => write!(f, "report has no snapshot object"),
            Self::MissingProducer => {
                write!(f, "report has no agent_id; pass --agent-id")
            }
            Self::MissingSinkBody { agent_id } => {
                write!(f, "sink line has no `body` field (agent_id=`{agent_id}`)")
            }
            Self::NotNumeric {
                resource_id,
                field,
                value,
            } => write!(
                f,
                "resource `{resource_id}`: `{field}` is not numeric: `{value}`"
            ),
            Self::Write { path, source } => write!(f, "write {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for AdapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Write { source, .. } => Some(source),
            _ => None,
        }
    }
}

// ── 输出：atlas.in.edge.v1 ──────────────────────────────────
//
// 字段**声明顺序即序列化顺序**。这不是洁癖：同一份快照的两次转换应该产出逐字节相同的
// JSON（可 diff、可做 golden 测试），所以顺序是契约的一部分，改动要同步改 fixture。

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeEnvelope {
    pub schema: String,
    pub source: Source,
    pub collect: Collect,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub system: String,
    pub producer: String,
    pub tenant: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collect {
    pub mode: String,
    pub snap_id: String,
    /// 快照**自己**说它是什么时候生成的。
    pub observed_at: String,
    /// **我们**收到它的时刻。两者不同：回放昨天的快照时 `observed_at` 是昨天。
    pub collected_at: String,
    pub res_ver: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
    pub hosts: Vec<Host>,
    pub interfaces: Vec<Interface>,
    pub processes: Vec<Process>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub hostname: String,
    /// agentd 的 `host.id` 原值（可能是 `hostname:<name>`）。
    pub external_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    /// 产品名（`macOS` / `Ubuntu`）。来自快照的 `os.name`。
    ///
    /// agentd 在 macOS 上特意发产品名而不是 sysinfo 的内核名（`Darwin`）—— 资产卡片
    /// 显示 `Darwin / 26.6.2` 没人看得懂。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_name: Option<String>,
    /// 来自快照的 `os.version`。Linux 上这是**内核**版本（sysinfo 语义）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    /// 来自快照的 `host.arch`（`aarch64` / `x86_64`）。包与二进制匹配靠它。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interface {
    pub host_ref: String,
    pub name: String,
    pub addresses: Vec<Address>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub family: String,
    pub ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cidr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Process {
    pub pid: i64,
    pub executable_name: String,
    pub host_ref: String,
    pub observed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
}

// ── 输入：agentd 的快照形状（容错） ─────────────────────────

/// `ReportDiscoverySnapshot`（`DSNAP:` 帧正文）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReportDocument {
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub revision: Option<i64>,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub reported_at: Option<String>,
    #[serde(default)]
    pub snapshot: Option<SnapshotDocument>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SnapshotDocument {
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub revision: Option<i64>,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub resources: Vec<SnapshotResource>,
    #[serde(default)]
    pub targets: Vec<SnapshotTarget>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SnapshotResource {
    #[serde(default)]
    pub resource_id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
    #[serde(default)]
    pub discovered_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SnapshotTarget {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub resource_ref: String,
    #[serde(default)]
    pub execution_hints: BTreeMap<String, Value>,
}

/// `state/discovery/meta.json`（`DiscoveryCacheMeta` 的容错投影）。
#[derive(Debug, Clone, Default, Deserialize)]
struct CacheMeta {
    #[serde(default)]
    snapshot_id: Option<String>,
    #[serde(default)]
    revision: Option<i64>,
    #[serde(default)]
    generated_at: Option<String>,
}

/// 信封里与「这份快照是谁、什么时候」有关的输入。
#[derive(Debug, Clone, Copy)]
pub struct SnapshotMeta<'a> {
    pub producer: &'a str,
    pub tenant: &'a str,
    pub env: Option<&'a str>,
    pub snapshot_id: &'a str,
    pub revision: i64,
    pub generated_at: &'a str,
    pub collected_at: &'a str,
}

// ── 转换 ────────────────────────────────────────────────────

/// 把一份快照的资源与目标列表转成 `atlas.in.edge.v1` 信封。
pub fn build_envelope(
    resources: &[SnapshotResource],
    targets: &[SnapshotTarget],
    meta: SnapshotMeta<'_>,
) -> Result<EdgeEnvelope, AdapterError> {
    let (host, host_name, machine_id) = build_host(resources)?;
    let interfaces = build_interfaces(resources, &host_name, machine_id.as_deref())?;
    let processes = build_processes(
        resources,
        targets,
        &host_name,
        machine_id.as_deref(),
        meta.generated_at,
    )?;

    Ok(EdgeEnvelope {
        schema: EDGE_SCHEMA.to_string(),
        source: Source {
            system: "wist".to_string(),
            producer: meta.producer.to_string(),
            tenant: meta.tenant.to_string(),
            env: meta.env.map(str::to_string),
        },
        collect: Collect {
            mode: "snapshot".to_string(),
            snap_id: meta.snapshot_id.to_string(),
            observed_at: meta.generated_at.to_string(),
            collected_at: meta.collected_at.to_string(),
            res_ver: meta.revision.to_string(),
        },
        payload: Payload {
            hosts: vec![host],
            interfaces,
            processes,
        },
    })
}

/// 解析一帧 `DSNAP:` 正文并转换。`agent_id` 为空时用快照自带的。
pub fn envelope_from_report_json(
    json: &str,
    agent_id: Option<&str>,
    tenant: &str,
    env: Option<&str>,
) -> Result<EdgeEnvelope, AdapterError> {
    let path = PathBuf::from("<report>");
    let report: ReportDocument =
        serde_json::from_str(json).map_err(|source| AdapterError::Parse {
            path: path.clone(),
            source,
        })?;
    envelope_from_report(&report, agent_id, tenant, env)
}

/// 网关 `agent-discovery` sink 落下的一行（wparse 的 OML 信封）→ `atlas.in.edge.v1`。
///
/// 和 [`envelope_from_report_json`] 的区别只在一层：那里的输入是 `DSNAP:` 帧的**裸正文**，
/// 而 sink 行把正文又套了一层 JSON 字符串（`body` 字段）。多出来的信封字段
/// （`category` / `biz` / `sink` / `wp_oml_name`）是给网关排查用的，转换不需要。
///
/// 字段取舍：
/// - `producer` 优先用正文里的 `agent_id`，缺失时才用行上的（两者本应一致）；
/// - `env` 优先用调用方给的 `env`，否则用行上的 —— CLI 的显式配置比部署默认值可信。
pub fn envelope_from_sink_line(
    line: &str,
    tenant: &str,
    env_override: Option<&str>,
) -> Result<EdgeEnvelope, AdapterError> {
    let path = PathBuf::from("<sink-line>");
    let row: SinkLine = serde_json::from_str(line).map_err(|source| AdapterError::Parse {
        path: path.clone(),
        source,
    })?;

    let body = row
        .body
        .as_deref()
        .filter(|body| !body.trim().is_empty())
        .ok_or(AdapterError::MissingSinkBody {
            agent_id: row.agent_id.clone().unwrap_or_default(),
        })?;

    let env = env_override
        .map(str::to_string)
        .or_else(|| row.env.filter(|value| !value.trim().is_empty()));

    envelope_from_report_json(body, row.agent_id.as_deref(), tenant, env.as_deref())
}

/// `agent-discovery.jsonl` 的一行。只声明我们看的字段，其余忽略。
#[derive(Debug, Clone, Default, Deserialize)]
struct SinkLine {
    #[serde(default)]
    agent_id: Option<String>,
    /// `ReportDiscoverySnapshot` 的 JSON，**以字符串形式**嵌在行里。
    #[serde(default)]
    body: Option<String>,
    /// wparse 的部署环境（如 `dev`）。
    #[serde(default)]
    env: Option<String>,
}

/// 同 [`envelope_from_report_json`]，但输入是已解析的文档。
pub fn envelope_from_report(
    report: &ReportDocument,
    agent_id: Option<&str>,
    tenant: &str,
    env: Option<&str>,
) -> Result<EdgeEnvelope, AdapterError> {
    let snapshot = report
        .snapshot
        .as_ref()
        .ok_or(AdapterError::MissingReportSnapshot)?;

    let producer = agent_id
        .map(str::to_string)
        .or_else(|| report.agent_id.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or(AdapterError::MissingProducer)?;

    let fallback_now = now_rfc3339();
    let collected_at = non_empty(&report.reported_at).unwrap_or(&fallback_now);

    build_envelope(
        &snapshot.resources,
        &snapshot.targets,
        SnapshotMeta {
            producer: &producer,
            tenant,
            env,
            snapshot_id: snapshot
                .snapshot_id
                .as_deref()
                .or(report.snapshot_id.as_deref())
                .unwrap_or_default(),
            revision: snapshot.revision.or(report.revision).unwrap_or(0),
            generated_at: snapshot
                .generated_at
                .as_deref()
                .or(report.generated_at.as_deref())
                .unwrap_or(&fallback_now),
            collected_at,
        },
    )
}

/// 读 agentd 配置目录下的 `state/discovery/`。
///
/// `state_dir` 传 agentd 的 **state 根**（即包含 `discovery/` 的那一层）。
pub fn envelope_from_state_dir(
    state_dir: &Path,
    agent_id: &str,
    tenant: &str,
    env: Option<&str>,
) -> Result<EdgeEnvelope, AdapterError> {
    let discovery = state_dir.join("discovery");
    let resources: Vec<SnapshotResource> = read_json(&discovery.join("resources.json"))?;
    let targets: Vec<SnapshotTarget> = read_json(&discovery.join("targets.json"))?;
    let meta: CacheMeta = read_json(&discovery.join("meta.json"))?;

    let fallback_now = now_rfc3339();
    build_envelope(
        &resources,
        &targets,
        SnapshotMeta {
            producer: agent_id,
            tenant,
            env,
            snapshot_id: meta.snapshot_id.as_deref().unwrap_or_default(),
            revision: meta.revision.unwrap_or(0),
            generated_at: meta.generated_at.as_deref().unwrap_or(&fallback_now),
            collected_at: &fallback_now,
        },
    )
}

// ── 各部分 ─────────────────────────────────────────────────

fn build_host(
    resources: &[SnapshotResource],
) -> Result<(Host, String, Option<String>), AdapterError> {
    let resource = resources
        .iter()
        .find(|resource| resource.kind == HOST_KIND)
        .ok_or(AdapterError::MissingHost)?;

    let host_id =
        attr(&resource.attributes, "host.id").unwrap_or_else(|| resource.resource_id.clone());
    let host_name = attr(&resource.attributes, "host.name").unwrap_or_else(|| {
        host_id
            .strip_prefix(HOSTNAME_PREFIX)
            .unwrap_or(&host_id)
            .to_string()
    });
    let machine_id = is_real_machine_id(&host_id).then(|| host_id.clone());

    // `os.*` 是后加的：老快照没有这几个键，所以是可选的，不能因为缺失就判坏。
    let os_name = attr(&resource.attributes, "os.name");
    let os_version = attr(&resource.attributes, "os.version");
    let arch = attr(&resource.attributes, "host.arch");

    let host = Host {
        hostname: host_name.clone(),
        external_ref: host_id,
        machine_id: machine_id.clone(),
        os_name,
        os_version,
        arch,
    };
    Ok((host, host_name, machine_id))
}

fn build_interfaces(
    resources: &[SnapshotResource],
    host_name: &str,
    machine_id: Option<&str>,
) -> Result<Vec<Interface>, AdapterError> {
    // 有序去重：同名网卡后者覆盖前者，但位置保持首次出现的位置
    // （与 Python dict 的语义一致；网卡数是个位到十位数，线性扫描够用，不值得引入 IndexMap）。
    let mut interfaces: Vec<(String, Interface)> = Vec::new();

    for resource in resources
        .iter()
        .filter(|resource| resource.kind == IFACE_KIND)
    {
        let Some(name) = attr(&resource.attributes, "net.if.name") else {
            continue;
        };
        let interface = Interface {
            host_ref: host_name.to_string(),
            name: name.clone(),
            addresses: Vec::new(),
            machine_id: machine_id.map(str::to_string),
            mac: attr(&resource.attributes, "net.if.mac"),
        };
        upsert(&mut interfaces, name, interface);
    }

    for resource in resources
        .iter()
        .filter(|resource| resource.kind == ADDR_KIND)
    {
        let Some(name) = attr(&resource.attributes, "net.if.name") else {
            continue;
        };
        if !meaningful_address(&resource.attributes) {
            continue;
        }
        let ip = attr(&resource.attributes, "net.if.addr").unwrap_or_default();

        let position = match interfaces
            .iter()
            .position(|(existing, _)| *existing == name)
        {
            Some(position) => position,
            None => {
                // 探针可能只产出 ip_address（没有单独的 network_interface 资源）。
                interfaces.push((
                    name.clone(),
                    Interface {
                        host_ref: host_name.to_string(),
                        name: name.clone(),
                        addresses: Vec::new(),
                        machine_id: machine_id.map(str::to_string),
                        mac: None,
                    },
                ));
                interfaces.len() - 1
            }
        };

        let prefix = match attr(&resource.attributes, "net.if.prefix") {
            Some(raw) => Some(
                raw.trim()
                    .parse::<i64>()
                    .map_err(|_| AdapterError::NotNumeric {
                        resource_id: resource.resource_id.clone(),
                        field: "net.if.prefix",
                        value: raw,
                    })?,
            ),
            None => None,
        };
        let address = Address {
            family: if ip.contains(':') { "ipv6" } else { "ipv4" }.to_string(),
            ip,
            prefix,
            cidr: attr(&resource.attributes, "net.if.cidr"),
            gateway: attr(&resource.attributes, "net.if.gateway"),
        };
        interfaces[position].1.addresses.push(address);
    }

    Ok(interfaces
        .into_iter()
        .map(|(_, interface)| interface)
        .filter(|interface| !interface.addresses.is_empty())
        .collect())
}

fn build_processes(
    resources: &[SnapshotResource],
    targets: &[SnapshotTarget],
    host_name: &str,
    machine_id: Option<&str>,
    generated_at: &str,
) -> Result<Vec<Process>, AdapterError> {
    // `process.identity` **不在** resource attributes 里 —— 它在 `targets[].execution_hints`。
    // 只读 resource 会让同 pid 的复用进程互相覆盖。
    let identity_by_ref: BTreeMap<&str, String> = targets
        .iter()
        .filter(|target| target.kind == PROCESS_KIND)
        .filter_map(|target| {
            let identity = attr(&target.execution_hints, "process.identity")?;
            Some((target.resource_ref.as_str(), identity))
        })
        .collect();

    let mut processes: Vec<((i64, String), Process)> = Vec::new();

    for resource in resources
        .iter()
        .filter(|resource| resource.kind == PROCESS_KIND)
    {
        let (Some(pid_raw), Some(executable)) = (
            attr(&resource.attributes, "process.pid"),
            attr(&resource.attributes, "process.executable.name"),
        ) else {
            continue;
        };
        let pid = pid_raw
            .trim()
            .parse::<i64>()
            .map_err(|_| AdapterError::NotNumeric {
                resource_id: resource.resource_id.clone(),
                field: "process.pid",
                value: pid_raw.clone(),
            })?;

        let identity = identity_by_ref.get(resource.resource_id.as_str()).cloned();
        // 没写 discovered_at 时退回快照的 generated_at：进程的观测时刻不能是空串，
        // 否则中心侧按 `observed_at` 排序时这条会排到最前。
        let observed_at = if resource.discovered_at.trim().is_empty() {
            generated_at.to_string()
        } else {
            resource.discovered_at.clone()
        };

        let key = (pid, identity.clone().unwrap_or_default());
        let process = Process {
            pid,
            executable_name: executable,
            host_ref: host_name.to_string(),
            observed_at,
            machine_id: machine_id.map(str::to_string),
            identity,
        };
        upsert(&mut processes, key, process);
    }

    let mut processes: Vec<Process> = processes.into_iter().map(|(_, p)| p).collect();
    processes.sort_by(|left, right| {
        left.pid
            .cmp(&right.pid)
            .then_with(|| left.executable_name.cmp(&right.executable_name))
    });
    Ok(processes)
}

/// 这个地址值不值得进资产目录。
///
/// 回环、链路本地、`169.254/16` 都不表达拓扑关系：留着只会让中心侧多出一堆
/// 「每台机都有一个 127.0.0.1」的噪声边。前缀比较统一转小写 —— IPv6 十六进制
/// 在不同探针上大小写不一致，`FE80:` 和 `fe80:` 是同一个东西。
fn meaningful_address(attributes: &BTreeMap<String, Value>) -> bool {
    let Some(ip) = attr(attributes, "net.if.addr") else {
        return false;
    };
    if matches!(
        attr(attributes, "net.if.addr.scope").as_deref(),
        Some("link_local") | Some("host")
    ) {
        return false;
    }
    let lowered = ip.to_ascii_lowercase();
    !(lowered.starts_with("127.")
        || lowered == "::1"
        || lowered.starts_with("fe80:")
        || lowered.starts_with("169.254."))
}

fn is_real_machine_id(host_id: &str) -> bool {
    !host_id.is_empty() && !host_id.starts_with(HOSTNAME_PREFIX)
}

/// 读一个属性值。契约里 attributes 是 `String -> String`，但数字也收 ——
/// 生产端把 pid 写成 `1` 而不是 `"1"` 是常见手误，为这个把整帧打回不值得。
fn attr(map: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    match map.get(key)? {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

fn upsert<K: PartialEq, V>(items: &mut Vec<(K, V)>, key: K, value: V) {
    match items.iter_mut().find(|(existing, _)| *existing == key) {
        Some(slot) => slot.1 = value,
        None => items.push((key, value)),
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, AdapterError> {
    let raw = fs::read_to_string(path).map_err(|source| AdapterError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| AdapterError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests;
