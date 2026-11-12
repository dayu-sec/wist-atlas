//! `atlas.in.inventory.v1`：云账号资产快照（`wist-harvest`）→ atlas 输入信封。
//!
//! ## 输入
//!
//! wparse `cloud-inventory` sink 分组落下的一行（与 agent-dsnap 同构）：
//!
//! ```json
//! {"schema":"atlas.in.inventory.v1","agent":"tencent-cloud-prod","ts":"…","seq":1,
//!  "body":"{\"schema_native\":\"wist.harvest.snapshot.v1\",\"assets\":{…}}"}
//! ```
//!
//! `body` 是**字符串**——快照原文。这是刻意的：数据面只搬契约对象，结构由消费者解析。
//!
//! ## 输出
//!
//! 标准 `atlas.in.inventory.v1` 信封。`payload` 特意对齐**已有的**抽取器形状
//! （`payload.hosts[]` / `payload.network_segments[]`），这样物料化层一行都不用改：
//!
//! | 云对象 | payload 位置 | 对应抽取器 |
//! | --- | --- | --- |
//! | CLOUD_HOST | `hosts[]` | `extract_host_candidates` |
//! | VPC / SUB_NET | `network_segments[]`（按 `cidr`） | `extract_network_segment_candidates` |
//!
//! ## 本版明确不做
//!
//! - **不建 `NetworkDomain`**：现在物料化层只有一个自动创建的 `default` 域，
//!   VPC→Domain 的语义需要另做（涉及建模决定），本版先把 VPC/子网都作为**段**送进来。
//! - **不做跨来源合并**：云主机与 agentd 上报的同一台机器靠
//!   `cloud_instance_id` 关联，那需要 `HostCandidate` 多一个字段 + 存储列 + resolver 改动。
//!   这里先把 `cloud_instance_id` 放进 host 行（当前的抽取器会忽略它，但字段已经在线上，
//!   等抽取器学会读它时不用再改数据面）。

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

/// 本适配器负责的 schema。
pub const INVENTORY_SCHEMA: &str = "atlas.in.inventory.v1";
/// 信封里的 `source.system`：**产品名**，换实现（Java → Rust）不变。
const SOURCE_SYSTEM: &str = "wist-harvest";

#[derive(Debug, Default, Deserialize)]
struct SinkLine {
    /// 采集器的生产者标识（wparse 记录里的 `agent`，即 `source_code`）。
    #[serde(default, alias = "agent_id")]
    agent: Option<String>,
    #[serde(default)]
    ts: Option<String>,
    #[serde(default)]
    env: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Snapshot {
    #[serde(default)]
    snap_id: String,
    #[serde(default)]
    collected_at: String,
    #[serde(default)]
    source_code: String,
    #[serde(default)]
    assets: Assets,
}

#[derive(Debug, Default, Deserialize)]
struct Assets {
    #[serde(default, rename = "CLOUD_HOST")]
    hosts: Vec<HostRow>,
    #[serde(default, rename = "VPC")]
    vpcs: Vec<NetRow>,
    #[serde(default, rename = "SUB_NET")]
    subnets: Vec<NetRow>,
}

#[derive(Debug, Default, Deserialize)]
struct HostRow {
    #[serde(default)]
    source_asset_id: String,
    #[serde(default)]
    asset_name: String,
    #[serde(default)]
    host_id: String,
    #[serde(default)]
    os_name: Option<String>,
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    arch: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct NetRow {
    #[serde(default)]
    source_asset_id: String,
    #[serde(default)]
    asset_name: String,
    #[serde(default)]
    cidr: Option<String>,
}

/// 一行 sink 记录 → `atlas.in.inventory.v1` 信封（`Value` 形态，调用方再转类型）。
pub fn envelope_from_sink_line(
    line: &str,
    tenant: &str,
    env_override: Option<&str>,
) -> Result<Value, String> {
    let row: SinkLine = serde_json::from_str(line)
        .map_err(|err| format!("inventory sink line is not JSON: {err}"))?;
    let body = row
        .body
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty())
        .ok_or_else(|| "inventory sink line has no `body`".to_string())?;
    let snapshot: Snapshot =
        serde_json::from_str(body).map_err(|err| format!("inventory body is not JSON: {err}"))?;

    if snapshot.snap_id.trim().is_empty() {
        return Err("inventory snapshot has no snap_id".to_string());
    }

    let producer = row
        .agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let code = snapshot.source_code.trim();
            if code.is_empty() { None } else { Some(code) }
        })
        .ok_or_else(|| "inventory snapshot has neither `agent` nor `source_code`".to_string())?;

    // 采集时间：优先用采集器记的（与 spool 一致），解析不了才退回当前时间。
    // **不要**用 `ts`（那是 wparse 记录时间，与快照的采集时间不是一回事）。
    let observed_at = parse_rfc3339(&snapshot.collected_at)
        .or_else(|| parse_rfc3339(row.ts.as_deref().unwrap_or_default()))
        .unwrap_or_else(now_rfc3339);

    let env = env_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            row.env
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    let hosts: Vec<Value> = snapshot
        .assets
        .hosts
        .iter()
        .filter(|host| !host.source_asset_id.trim().is_empty())
        .map(|host| {
            let name = if host.asset_name.trim().is_empty() {
                host.host_id.as_str()
            } else {
                host.asset_name.as_str()
            };
            json!({
                // 抽取器读 `host_name` / `hostname` 两种写法，这里给语义最直白的那个。
                "host_name": name,
                // 云侧外部 ID：资源匹配与后续跨来源归并都以它为准。
                "external_ref": host.source_asset_id,
                // 跨来源归并键。**当前抽取器还不读它**（`HostCandidate` 没有这个字段），
                // 但先放到线上：等字段加上时数据面不用再改。
                "cloud_instance_id": host.source_asset_id,
                "os_name": host.os_name,
                "ip": host.ip,
                "arch": host.arch,
            })
        })
        .collect();

    let mut segments: Vec<Value> = Vec::new();
    for row in snapshot
        .assets
        .vpcs
        .iter()
        .chain(snapshot.assets.subnets.iter())
    {
        // 抽取器要求 `cidr` 或 `ip_addr` 至少有一个，两者都空的行会被整批打回。
        let Some(cidr) = row.cidr.as_deref().map(str::trim).filter(|c| !c.is_empty()) else {
            continue;
        };
        let name = if row.asset_name.trim().is_empty() {
            row.source_asset_id.as_str()
        } else {
            row.asset_name.as_str()
        };
        segments.push(json!({
            "segment_name": name,
            "cidr": cidr,
            // 云侧网络 ID：后续做 VPC→NetworkDomain 时要用。
            "external_ref": row.source_asset_id,
        }));
    }

    if hosts.is_empty() && segments.is_empty() {
        return Err("inventory snapshot carries neither hosts nor network segments".to_string());
    }

    Ok(json!({
        "schema": INVENTORY_SCHEMA,
        "source": {
            "system": SOURCE_SYSTEM,
            "producer": producer,
            "tenant": tenant,
            "env": env,
        },
        "collect": {
            "mode": "snapshot",
            "snap_id": snapshot.snap_id,
            "observed_at": observed_at,
            "collected_at": observed_at,
        },
        "payload": {
            "hosts": hosts,
            "network_segments": segments,
        },
    }))
}

fn parse_rfc3339(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let parsed = DateTime::parse_from_rfc3339(raw).ok()?;
    Some(
        parsed
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Micros, true),
    )
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink_line(body: &str) -> String {
        json!({
            "schema": INVENTORY_SCHEMA,
            "agent": "tencent-cloud-prod",
            "ts": "2026-11-12T10:35:13Z",
            "seq": 1,
            "body": body,
        })
        .to_string()
    }

    fn snapshot() -> String {
        json!({
            "schema_native": "wist.harvest.snapshot.v1",
            "snap_id": "tencent-cloud-prod-inventory-abc-20261112T103513Z",
            "collected_at": "2026-11-12T18:35:13+08:00",
            "source_code": "tencent-cloud-prod",
            "assets": {
                "CLOUD_HOST": [
                    {"source_asset_id":"ins-1","asset_name":"order-api-01","os_name":"Ubuntu 22.04 LTS","ip":"10.0.1.10","arch":null},
                    {"source_asset_id":"ins-2","asset_name":"","host_id":"ins-2","os_name":null}
                ],
                "VPC": [{"source_asset_id":"vpc-1","asset_name":"prod-vpc","cidr":"10.0.0.0/16"}],
                "SUB_NET": [
                    {"source_asset_id":"subnet-1","asset_name":"prod-a","cidr":"10.0.1.0/24"},
                    {"source_asset_id":"subnet-2","asset_name":"无网段","cidr":""}
                ]
            }
        })
        .to_string()
    }

    #[test]
    fn maps_hosts_and_segments() {
        let value = envelope_from_sink_line(&sink_line(&snapshot()), "DEV-X4", None).unwrap();
        assert_eq!(value["schema"], INVENTORY_SCHEMA);
        assert_eq!(value["source"]["system"], "wist-harvest");
        assert_eq!(value["source"]["producer"], "tencent-cloud-prod");
        assert_eq!(value["source"]["tenant"], "DEV-X4");
        assert_eq!(value["collect"]["mode"], "snapshot");

        let hosts = value["payload"]["hosts"].as_array().unwrap();
        assert_eq!(hosts.len(), 2, "两台主机都要保留");
        assert_eq!(hosts[0]["host_name"], "order-api-01");
        assert_eq!(hosts[0]["external_ref"], "ins-1");
        assert_eq!(hosts[0]["cloud_instance_id"], "ins-1");
        // 名称为空时回退 host_id —— 不能送一个空 hostname（抽取器会拒）
        assert_eq!(hosts[1]["host_name"], "ins-2");

        let segments = value["payload"]["network_segments"].as_array().unwrap();
        assert_eq!(segments.len(), 2, "cidr 为空的段要丢掉，否则整批被拒");
        assert_eq!(segments[0]["cidr"], "10.0.0.0/16");
        assert_eq!(segments[1]["cidr"], "10.0.1.0/24");
    }

    #[test]
    fn normalizes_collected_at_to_utc() {
        let value = envelope_from_sink_line(&sink_line(&snapshot()), "DEV-X4", None).unwrap();
        assert_eq!(
            value["collect"]["observed_at"],
            "2026-11-12T10:35:13.000000Z"
        );
    }

    #[test]
    fn rejects_missing_body_and_snap_id() {
        let no_body = json!({"schema": INVENTORY_SCHEMA, "agent": "a"}).to_string();
        assert!(envelope_from_sink_line(&no_body, "t", None).is_err());

        let no_snap = json!({"snap_id": "", "assets": {"CLOUD_HOST": []}}).to_string();
        assert!(envelope_from_sink_line(&sink_line(&no_snap), "t", None).is_err());
    }

    #[test]
    fn rejects_snapshot_without_any_asset() {
        let empty =
            json!({"snap_id": "s1", "assets": {"CLOUD_HOST": [], "VPC": [], "SUB_NET": []}})
                .to_string();
        let err = envelope_from_sink_line(&sink_line(&empty), "t", None).unwrap_err();
        assert!(err.contains("neither hosts nor network segments"), "{err}");
    }

    #[test]
    fn producer_falls_back_to_source_code() {
        let line = json!({"schema": INVENTORY_SCHEMA, "body": snapshot()}).to_string();
        let value = envelope_from_sink_line(&line, "t", None).unwrap();
        assert_eq!(value["source"]["producer"], "tencent-cloud-prod");
    }
}
