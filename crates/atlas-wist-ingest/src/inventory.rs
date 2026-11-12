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
//! ## 完整性声明（`collect.coverage`）
//!
//! 采集器在快照里写 `coverage: { scope, complete, account }` 时，原样透传到信封。
//! **这是主机退役的唯一凭证**：只有 `scope=account && complete && account` 的帧，
//! 缺席才能当作「云上已不存在」。`account` 同时写进每条主机行的 `cloud_account` ——
//! 退役目标靠这两处相等来圈定。
//!
//! 采集器不给就不给：缺省 = 只 upsert、永不判死。**不要在这里替它编一个账号键**
//! （比如拿 `source_code` 顶替），否则哪天采集器改用真 `AccountId`，存量行会永久失配。
//!
//! ## 本版明确不做
//!
//! - **不建 `NetworkDomain`**：现在物料化层只有一个自动创建的 `default` 域，
//!   VPC→Domain 的语义需要另做（涉及建模决定），本版先把 VPC/子网都作为**段**送进来。
//! - **不送 EIP**：EIP 的生命周期与主机独立（机器下线 EIP 还在），是独立对象，
//!   等模型定名后单独接。

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
    /// 采集器对「这帧能不能当全集用」的声明。缺省 = 没声明 = 不能当全集。
    #[serde(default)]
    coverage: Option<CoverageDecl>,
}

/// `{"scope":"account","complete":true,"account":"tencent:1300000000"}`。
///
/// `account` 是**全键**（`vendor:account`）：atlas 只拿它做相等比较，不解析。
/// 缺账号就不能退役 —— 不知道该扫哪些主机，拿它去扫整个租户是灾难。
#[derive(Debug, Default, Deserialize)]
struct CoverageDecl {
    #[serde(default)]
    scope: String,
    #[serde(default)]
    complete: bool,
    #[serde(default)]
    account: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Assets {
    #[serde(default, rename = "CLOUD_HOST")]
    hosts: Vec<HostRow>,
    #[serde(default, rename = "EIP")]
    eips: Vec<EipRow>,
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
    /// 所属子网 ID（兼容字段名）。
    #[serde(default)]
    subvpcid: String,
}

#[derive(Debug, Default, Deserialize)]
struct EipRow {
    #[serde(default)]
    source_asset_id: String,
    #[serde(default)]
    asset_name: String,
    #[serde(default)]
    eip_address_ip: String,
    #[serde(default)]
    eip_address_status: String,
    /// 目标类型（`CVM` / `NAT` / `CLB`）。
    #[serde(default)]
    eip_instance_type: String,
    /// 目标 ID（`InstanceId`）。解绑后为空。
    #[serde(default)]
    eip_instance_id: String,
    #[serde(default)]
    eip_bandwidth: i64,
    #[serde(default)]
    eip_charge_type: String,
}

#[derive(Debug, Default, Deserialize)]
struct NetRow {
    #[serde(default)]
    source_asset_id: String,
    #[serde(default)]
    asset_name: String,
    #[serde(default)]
    cidr: Option<String>,
    /// 子网所属 VPC（兼容字段名 `parent_source_asset_id`）。
    #[serde(default)]
    parent_source_asset_id: String,
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

    // 账号键：以采集器声明为准。它同时写进 `collect.coverage.account`（帧级）和各主机行，
    // 两处用同一个值——退役判定靠它们相等。采集器不给就不给：宁可退不了役，
    // 也不要自己编一个键出来把别人的机器扫掉。
    let account = snapshot
        .coverage
        .as_ref()
        .and_then(|coverage| coverage.account.as_deref())
        .map(str::trim)
        .filter(|account| !account.is_empty())
        .map(str::to_string);

    // 声明的归一化：认不出来的 `scope` 一律降级成 `partial`（少退役，不误删）。
    let coverage_value = snapshot.coverage.as_ref().map(|coverage| {
        let scope = if coverage.scope.trim().eq_ignore_ascii_case("account") {
            "account"
        } else {
            "partial"
        };
        let mut value = json!({ "scope": scope, "complete": coverage.complete });
        if let Some(account) = account.as_ref() {
            value["account"] = json!(account);
        }
        value
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
                // 跨来源归并键：agentd 从元数据服务读到的 instance-id 与它相等即同一台机。
                "cloud_instance_id": host.source_asset_id,
                // 账号归属：退役判定的作用域（云侧独占写）。
                "cloud_account": account,
                "os_name": host.os_name,
                "ip": host.ip,
                // 私网 IP 与所属子网：拓扑里「这台机在哪个子网」靠这两个。
                "ip_addr": host.ip,
                "subnet_external_ref": host.subvpcid,
                "arch": host.arch,
            })
        })
        .collect();

    // EIP：独立资产。地址 ID 是稳定键，公网地址是它的属性，绑定目标另存。
    let elastic_ips: Vec<Value> = snapshot
        .assets
        .eips
        .iter()
        .filter(|row| {
            !row.source_asset_id.trim().is_empty() && !row.eip_address_ip.trim().is_empty()
        })
        .map(|row| {
            let bound_ref = row.eip_instance_id.trim();
            json!({
                "external_ref": row.source_asset_id,
                "address": row.eip_address_ip,
                "status": row.eip_address_status,
                "charge_type": row.eip_charge_type,
                "bandwidth_mbps": row.eip_bandwidth,
                "cloud_account": account,
                "bound_instance_ref": if bound_ref.is_empty() { None } else { Some(bound_ref) },
                "bound_instance_type": row.eip_instance_type,
            })
        })
        .collect();

    // VPC 是**地址空间**（域），不是网段：塞进 network_segments 会只能靠 CIDR 猜归属。
    let domains: Vec<Value> = snapshot
        .assets
        .vpcs
        .iter()
        .filter(|row| !row.source_asset_id.trim().is_empty())
        .map(|row| {
            json!({
                "external_ref": row.source_asset_id,
                "name": row.asset_name,
                // VPC 的 CIDR 是地址空间本身：不存就只能每次回云上问。
                "cidr": row.cidr,
            })
        })
        .collect();

    let mut segments: Vec<Value> = Vec::new();
    for row in &snapshot.assets.subnets {
        // 抽取器要求 `cidr` 或 `ip_addr` 至少有一个，两者都空的行会被整批打回。
        let Some(cidr) = row.cidr.as_deref().map(str::trim).filter(|c| !c.is_empty()) else {
            continue;
        };
        let name = if row.asset_name.trim().is_empty() {
            row.source_asset_id.as_str()
        } else {
            row.asset_name.as_str()
        };
        let parent = row.parent_source_asset_id.trim();
        segments.push(json!({
            "segment_name": name,
            "cidr": cidr,
            // 子网 ID 是段的稳定键；所属 VPC ID 让它稳定挂进对应的域。
            "external_ref": row.source_asset_id,
            "parent_external_ref": if parent.is_empty() { Value::Null } else { json!(parent) },
        }));
    }

    if hosts.is_empty() && domains.is_empty() && segments.is_empty() && elastic_ips.is_empty() {
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
            // 采集器没声明就不带这个键：缺席与 `partial` 同义，但下游能看出
            // 「这个采集器还没升级」，而不是以为它宣称了不完整。
            "coverage": coverage_value.unwrap_or(Value::Null),
        },
        "payload": {
            "hosts": hosts,
            "network_domains": domains,
            // 键必须在：`reported` 决定能不能拿「没有它」去退役 EIP。
            // 采集器把 EIP 关掉时这里是空数组，下游据此跳过退役比对。
            "elastic_ips": elastic_ips,
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
                    {"source_asset_id":"ins-1","asset_name":"order-api-01","os_name":"Ubuntu 22.04 LTS","ip":"10.0.1.10","arch":null,"subvpcid":"subnet-1"},
                    {"source_asset_id":"ins-2","asset_name":"","host_id":"ins-2","os_name":null}
                ],
                "VPC": [{"source_asset_id":"vpc-1","asset_name":"prod-vpc","cidr":"10.0.0.0/16"}],
                "SUB_NET": [
                    {"source_asset_id":"subnet-1","asset_name":"prod-a","cidr":"10.0.1.0/24",
                     "parent_source_asset_id":"vpc-1"},
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

        // VPC 是地址空间（域），子网才是网段，两者分开送。
        let domains = value["payload"]["network_domains"].as_array().unwrap();
        assert_eq!(domains.len(), 1, "一个 VPC 一个域");
        assert_eq!(domains[0]["external_ref"], "vpc-1");
        assert_eq!(domains[0]["name"], "prod-vpc");
        assert_eq!(
            domains[0]["cidr"], "10.0.0.0/16",
            "VPC 的地址空间要随域一起送"
        );

        let segments = value["payload"]["network_segments"].as_array().unwrap();
        assert_eq!(segments.len(), 1, "cidr 为空的段要丢掉，否则整批被拒");
        assert_eq!(segments[0]["cidr"], "10.0.1.0/24");
        assert_eq!(
            segments[0]["external_ref"], "subnet-1",
            "子网 ID 是段的稳定键"
        );
        assert_eq!(
            segments[0]["parent_external_ref"], "vpc-1",
            "子网要能稳定挂进对应的域"
        );
    }

    #[test]
    fn passes_through_the_coverage_declaration_and_account_key() {
        // 没有这个声明，atlas 就只能二选一：把采集不全当销毁，或者永不退役。
        let mut snapshot: Value = serde_json::from_str(&snapshot()).unwrap();
        snapshot["coverage"] = json!({
            "scope": "account",
            "complete": true,
            "account": "tencent:1300000000"
        });

        let value =
            envelope_from_sink_line(&sink_line(&snapshot.to_string()), "DEV-X4", None).unwrap();
        assert_eq!(value["collect"]["coverage"]["scope"], "account");
        assert_eq!(value["collect"]["coverage"]["complete"], true);
        assert_eq!(
            value["collect"]["coverage"]["account"],
            "tencent:1300000000"
        );
        // 主机行也要带账号键：退役判定靠它与帧级声明相等。
        assert_eq!(
            value["payload"]["hosts"][0]["cloud_account"],
            "tencent:1300000000"
        );
    }

    #[test]
    fn does_not_invent_coverage_when_the_collector_omits_it() {
        // 采集器没声明就什么都不说：宁可退不了役，也不要自己编一个账号键。
        let value = envelope_from_sink_line(&sink_line(&snapshot()), "DEV-X4", None).unwrap();
        assert!(value["collect"]["coverage"].is_null());
        assert!(value["payload"]["hosts"][0]["cloud_account"].is_null());
    }

    #[test]
    fn an_unknown_coverage_scope_degrades_to_partial() {
        let mut snapshot: Value = serde_json::from_str(&snapshot()).unwrap();
        snapshot["coverage"] = json!({"scope": "everything", "complete": true});
        let value =
            envelope_from_sink_line(&sink_line(&snapshot.to_string()), "DEV-X4", None).unwrap();
        assert_eq!(
            value["collect"]["coverage"]["scope"], "partial",
            "认不出来的 scope 一律降级：少退役，不误删"
        );
    }

    #[test]
    fn maps_eips_as_independent_assets() {
        // EIP 不是主机的字段：地址 ID 是身份，绑定目标是快照态。
        let mut snapshot: Value = serde_json::from_str(&snapshot()).unwrap();
        snapshot["coverage"] = json!({
            "scope": "account",
            "complete": true,
            "account": "tencent:1300000000"
        });
        snapshot["assets"]["EIP"] = json!([
            {
                "source_asset_id": "eip-1",
                "asset_name": "prod-eip",
                "eip_address_ip": "203.0.113.10",
                "eip_address_status": "BIND",
                "eip_instance_type": "CVM",
                "eip_instance_id": "ins-1",
                "eip_bandwidth": 10,
                "eip_charge_type": "TRAFFIC_POSTPAID_BY_HOUR"
            },
            {
                "source_asset_id": "eip-2",
                "asset_name": "",
                "eip_address_ip": "203.0.113.11",
                "eip_address_status": "UNBIND",
                "eip_instance_type": "",
                "eip_instance_id": "",
                "eip_bandwidth": 5,
                "eip_charge_type": "BANDWIDTH_PREPAID"
            }
        ]);

        let value =
            envelope_from_sink_line(&sink_line(&snapshot.to_string()), "DEV-X4", None).unwrap();
        let ips = value["payload"]["elastic_ips"].as_array().unwrap();
        assert_eq!(ips.len(), 2);
        assert_eq!(ips[0]["external_ref"], "eip-1");
        assert_eq!(ips[0]["address"], "203.0.113.10");
        assert_eq!(ips[0]["status"], "BIND");
        assert_eq!(ips[0]["bound_instance_ref"], "ins-1");
        assert_eq!(ips[0]["bound_instance_type"], "CVM");
        assert_eq!(
            ips[0]["cloud_account"], "tencent:1300000000",
            "账号键同时写进 EIP：退役判定靠它圈定目标"
        );
        assert!(
            ips[1]["bound_instance_ref"].is_null(),
            "解绑的 EIP 不能带目标"
        );
    }

    #[test]
    fn the_elastic_ips_key_is_present_even_when_empty() {
        // 键必须在：下游据此判断「本帧观测了 EIP」，缺席与空数组语义不同。
        let value = envelope_from_sink_line(&sink_line(&snapshot()), "DEV-X4", None).unwrap();
        assert_eq!(
            value["payload"]["elastic_ips"],
            json!([]),
            "采集器没采到 EIP 时也要给空数组"
        );
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
