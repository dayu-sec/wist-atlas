# wist-atlas 输入分类、规范与样式

## 1. 文档目的

本文档基于 `doc/model` 下的模型设计，收敛第一版外部输入数据的分类、规范和 JSON 样式。

它回答三件事：

- 模型文档中到底有哪些输入类型
- 每类输入应该长成什么样
- raw input、candidate、source-of-truth model 之间如何分层

相关文档：

- [`README.md`](./README.md)
- [`external-glossary.md`](./external-glossary.md)
- [`../model/unified-topology-schema.md`](../model/unified-topology-schema.md)
- [`external-input-spec.md`](./external-input-spec.md)
- [`warp-insight-adapter-spec.md`](./warp-insight-adapter-spec.md)
- [`../architecture/dataflow-and-pipeline-architecture.md`](../architecture/dataflow-and-pipeline-architecture.md)

---

## 2. 分层口径

第一版统一按下面链路理解所有输入：

```text
external raw input
  -> staged payload
  -> source-specific adapter
  -> canonical cand / ev / obs
  -> identity resolution
  -> source-of-truth model
  -> read model / derived view
```

| 层 | 含义 | 示例 |
| --- | --- | --- |
| `external raw input` | 外部系统或采集器给出的原始结构化事实 | edge discovery、CMDB export、OSV advisory |
| `staged payload` | 中心暂存的原始载荷和元数据 | raw JSON、object storage ref、fetch metadata |
| `candidate` | 等待身份解析的对象候选 | `HostCand`、`SubjectCand` |
| `evidence` | 支撑某个判断的证据 | `SwEv`、`RtBindEv`、`DepEv` |
| `observation` | 从高频事实聚合出的观测摘要 | `DepObs`、`BugObs` |
| `source-of-truth` | 经过解析和幂等写入的中心主对象或关系 | `HostInventory`、`ServiceEntity`、`ResponsibilityAssignment` |
| `derived view` | 可重算查询视图 | topology view、risk view、effective responsibility |

约束：

- raw input 不能直接变成 source-of-truth。
- candidate / evidence / observation 必须保留来源和时间。
- identity resolution 只在中心语义层做，不下放给单个 connector。
- 无法解析的输入必须停留在 unresolved candidate / evidence，不写正式关系。

---

## 3. 输入分类矩阵

| 分类 ID | 输入分类 | 典型来源 | 主要中间层 | 目标模型 | 参考模型文档 |
| --- | --- | --- | --- | --- | --- |
| `IN-01` | 主机目录输入 | edge discovery、CMDB、云 API | `HostCand` | `HostInventory` | [`host-inventory-and-runtime-state.md`](./host-inventory-and-runtime-state.md) |
| `IN-02` | 主机运行态输入 | agent metrics、runtime snapshot | `RuntimeSnapshot` | `HostRuntimeState` | [`host-inventory-and-runtime-state.md`](./host-inventory-and-runtime-state.md) |
| `IN-03` | 主机/Pod 网络输入 | edge discovery、CNI、K8s、云网络 | `HostNetEv`、`PodNetEv` | `NetworkDomain`、`NetworkSegment`、`HostNetAssoc`、`PodNetAssoc` | [`host-pod-network-topology-model.md`](./host-pod-network-topology-model.md) |
| `IN-04` | 编排目录输入 | K8s API、Nomad API | `ClusterCand`、`WorkloadCand` | `ClusterInventory`、`NamespaceInventory`、`WorkloadEntity`、`PodInventory` | [`cluster-namespace-workload-topology-model.md`](./cluster-namespace-workload-topology-model.md) |
| `IN-05` | 业务/系统/服务目录输入 | CMDB、service catalog、manual import | `CatalogCand` | `BusinessDomain`、`SystemBoundary`、`Subsystem`、`ServiceEntity` | [`business-system-service-topology-model.md`](./business-system-service-topology-model.md) |
| `IN-06` | 服务入口与实例地址输入 | K8s Service、Ingress、mesh、LB、DNS | `EndpointCand` | `SvcEp`、`InstEp`、`EpRes` | [`endpoint-and-dependency-observation-model.md`](./endpoint-and-dependency-observation-model.md) |
| `IN-07` | 运行绑定输入 | K8s label、pod owner、process facts、manual override | `RtBindEv` | `ServiceInstance`、`RuntimeBinding` | [`runtime-binding-model.md`](./runtime-binding-model.md) |
| `IN-08` | 依赖观测输入 | trace、access log、flow、DNS、gateway log | `DepEv`、`DepObs` | `EpRes`、`DepEdge` | [`endpoint-and-dependency-observation-model.md`](./endpoint-and-dependency-observation-model.md) |
| `IN-09` | 软件识别输入 | process discovery、package inventory、file discovery、container image | `SwEv` | `SoftwareProduct`、`SoftwareVersion`、`SoftwareArtifact` | [`software-normalization-and-vuln-enrichment.md`](./software-normalization-and-vuln-enrichment.md) |
| `IN-10` | 制品验真输入 | hash scanner、signature verifier、registry metadata | `ArtifactVerifyCand` | `ArtifactVerification` | [`software-normalization-and-vuln-enrichment.md`](./software-normalization-and-vuln-enrichment.md) |
| `IN-11` | 漏洞情报输入 | OSV、GHSA、NVD、vendor advisory | `VulnAdvisoryRaw`、`FindingCand` | `SoftwareVulnerabilityFinding` | [`public-vulnerability-source-ingestion.md`](./public-vulnerability-source-ingestion.md) |
| `IN-12` | BUG / 错误信号输入 | error log、crash dump、panic、issue tracker | `BugEv`、`BugObs` | `SoftwareBug`、`SoftwareBugFinding` | [`software-normalization-and-vuln-enrichment.md`](./software-normalization-and-vuln-enrichment.md) |
| `IN-13` | 主体与组织输入 | LDAP、IAM、HR | `SubjectCand`、`SubjectMemberCand` | `Subject`、subject membership | [`host-responsibility-sync-from-external-systems.md`](./host-responsibility-sync-from-external-systems.md) |
| `IN-14` | 责任与值班输入 | CMDB、Oncall、manual governance | `RespCand`、`OncallCand` | `ResponsibilityAssignment`、effective responsibility view | [`host-responsibility-and-maintainer-model.md`](./host-responsibility-and-maintainer-model.md) |
| `IN-15` | 安全与风险输入 | EDR、scanner、IDS、policy engine | `ThreatEv`、`RiskCand` | `BusinessHealthFactor`、risk derived view | [`business-system-service-topology-model.md`](./business-system-service-topology-model.md) |
| `IN-16` | 人工修正输入 | admin UI、batch correction、approval workflow | `CorrectionCand` | 对应主对象或关系对象的版本化变更 | [`host-responsibility-and-maintainer-model.md`](./host-responsibility-and-maintainer-model.md) |

阶段取舍：

| 阶段 | 优先输入 |
| --- | --- |
| `P0` | `IN-01`、`IN-03`、`IN-13`、`IN-14` 的最小子集 |
| `P1` | `IN-04`、`IN-05`、`IN-06`、`IN-07` |
| `P2` | `IN-08`、`IN-09`、`IN-10`、`IN-11`、`IN-12`、`IN-15`、`IN-16` |

---

## 4. 通用 Envelope 样式

所有外部输入使用同一 envelope 样式：

```json
{
  "schema": "atlas.in.<family>.v1",
  "source": {
    "kind": "edge",
    "system": "warp-insight",
    "producer": "agent-01",
    "tenant_ref": "tenant-demo",
    "env_ref": "prod"
  },
  "collect": {
    "mode": "snapshot",
    "snap_id": "snap-001",
    "observed_at": "2026-04-26T02:20:30Z"
  },
  "payload": {}
}
```

推荐 `source_family`：

- `edge`
- `host_runtime`
- `cmdb`
- `iam`
- `oncall`
- `k8s`
- `telemetry`
- `sw`
- `artifact`
- `vuln`
- `bug_signal`
- `security`
- `manual_batch`
- `correction`

`collect.mode` 取值：

- `snapshot`
- `full`
- `incremental`
- `window`
- `correction`

---

## 5. JSON 样式规则

### 5.1 命名

固定规则：

- 字段名使用 `snake_case`。
- 枚举值使用小写字符串。
- 内部主键命名为 `*_id`。
- 外部 ID 命名为 `external_id` 或 `*_external_id`。
- 外部引用命名为 `external_ref` 或 `*_external_ref`。
- 时间字段使用明确后缀，例如 `observed_at`、`valid_from`、`win_start`。

禁止：

- 用 `id` 表示来源不明的标识。
- 同一字段有时填内部 UUID、有时填外部字符串。
- 在 raw input 中提前填写中心内部 UUID。

### 5.2 时间

固定规则：

- 所有时间使用 RFC3339 UTC，例如 `2026-04-26T02:20:30Z`。
- `observed_at` 表示事实被观察到的时间。
- `collected_at` 表示 agent 采集完成时间。
- `fetched_at` 表示 connector 拉取完成时间。
- `valid_from / valid_to` 表示关系或职责生效时间。
- `win_start / win_end` 只用于窗口聚合输入。

### 5.3 单位

数值必须在字段名中体现单位：

- 字节：`*_bytes`
- 秒：`*_seconds`
- 百分比：`*_pct`
- 毫秒：`*_ms`
- 计数：`*_count`

### 5.4 引用与证据

外部输入里引用对象时优先使用外部标识：

```json
{
  "service_external_ref": "svc-checkout-api",
  "team_external_id": "team-platform",
  "machine_id": "office-machine-002"
}
```

大体量原始数据不进入 topology 主库，统一使用 `evidence_ref` 或 `evidence_refs[]`：

```json
{
  "evidence_refs": [
    {
      "type": "trace_sample",
      "ref": "s3://bucket/path/file.ndjson"
    }
  ]
}
```

### 5.5 扩展字段

来源系统私有字段放入 `attributes`、`labels`、`annotations` 或 `raw_facts`。

规则：

- 稳定字段提升为一等字段。
- 暂不稳定字段放入扩展对象。
- 不要把所有字段都塞进 `metadata`。

---

## 6. 各类输入样式

### 6.1 主机目录输入 `IN-01`

payload 样式：

```json
{
  "hosts": [
    {
      "hostname": "office-build-01",
      "fqdn": "office-build-01.corp.example.com",
      "machine_id": "office-machine-002",
      "cloud_instance_id": "i-001",
      "os": {
        "name": "linux",
        "version": "6.8.0"
      }
    }
  ]
}
```

adapter 输出：

- `HostCand`

identity 优先级：

1. `machine_id`
2. `cloud_instance_id`
3. `(tenant_external_ref, hostname)`

### 6.2 主机运行态输入 `IN-02`

payload 样式：

```json
{
  "host": {
    "hostname": "office-build-01",
    "machine_id": "office-machine-002"
  },
  "runtime_state": {
    "observed_at": "2026-04-26T02:20:30Z",
    "boot_id": "7f7ac860-8f66-46f1-9f4f-33b3c81cbbee",
    "uptime_seconds": 345923,
    "cpu_usage_pct": 32.4,
    "memory_used_bytes": 8589934592,
    "process_count": 212,
    "container_count": 17,
    "agent_health": "healthy"
  }
}
```

adapter 输出：

- `RuntimeSnapshot`
- resolved 后写 `HostRuntimeState`

### 6.3 网络输入 `IN-03`

payload 样式：

```json
{
  "network_interfaces": [
    {
      "name": "eth0",
      "mac": "02:42:c0:a8:0a:34",
      "addresses": [
        {
          "family": "ipv4",
          "ip": "192.168.10.52",
          "prefix": 24,
          "gateway": "192.168.10.1"
        }
      ]
    }
  ]
}
```

adapter 输出：

- `HostNetEv`
- `NetSegCand`
- `HostNetAssocCand`

### 6.4 编排目录输入 `IN-04`

payload 样式：

```json
{
  "cluster": {
    "external_id": "cluster-office-k8s",
    "name": "office-k8s"
  },
  "namespaces": [],
  "workloads": [],
  "pods": [],
  "services": []
}
```

adapter 输出：

- `ClusterCand`
- `NamespaceCand`
- `WorkloadCand`
- `PodCand`
- `EndpointCand`

### 6.5 业务/系统/服务目录输入 `IN-05`

payload 样式：

```json
{
  "business_units": [],
  "systems": [],
  "services": [],
  "declared_dependencies": []
}
```

adapter 输出：

- `CatalogCand`
- `DeclDepCand`
- `RespCand`

### 6.6 Endpoint 输入 `IN-06`

payload 样式：

```json
{
  "service_endpoints": [
    {
      "service_external_ref": "svc-checkout-api",
      "endpoint_kind": "dns",
      "address": "checkout.example.com",
      "port": 443,
      "protocol": "https"
    }
  ],
  "instance_endpoints": [
    {
      "instance_key": "pod:f1cb0ff1-5f8d-45ac-b673-2ef0fd264a44",
      "address": "10.244.2.31",
      "port": 8080,
      "protocol": "http"
    }
  ]
}
```

adapter 输出：

- `EndpointCand`
- `EpResCand`

### 6.7 运行绑定输入 `IN-07`

payload 样式：

```json
{
  "runtime_bindings": [
    {
      "service_external_ref": "svc-checkout-api",
      "runtime_object": {
        "type": "pod",
        "pod_uid": "f1cb0ff1-5f8d-45ac-b673-2ef0fd264a44"
      },
      "scope": "declared",
      "confidence": "high",
      "evidence": [
        {
          "type": "k8s_label_match",
          "value": "atlas/service=svc-checkout-api"
        }
      ]
    }
  ]
}
```

adapter 输出：

- `RtBindEv`
- `RtBindCand`

### 6.8 依赖观测输入 `IN-08`

payload 样式：

```json
{
  "dependency_observations": [
    {
      "observation_type": "trace_span",
      "upstream": {
        "service_external_ref": "svc-checkout-api"
      },
      "downstream": {
        "address": "payment-db.payments.svc.cluster.local",
        "port": 5432
      },
      "sample_count": 1842,
      "first_observed_at": "2026-04-26T02:25:01Z",
      "last_observed_at": "2026-04-26T02:29:59Z"
    }
  ]
}
```

adapter 输出：

- `DepEv`
- `DepObs`
- `EpResCand`

生成 `DepEdge` 的规则：

- 不能由单条日志直接生成。
- 必须先窗口聚合成 `DepObs`。
- 地址经 `EpRes` 解析到 service / instance 后再刷新 `DepEdge`。

### 6.9 软件识别输入 `IN-09`

payload 样式：

```json
{
  "software_evidence": [
    {
      "resource_ref": {
        "type": "process",
        "pid": 1834
      },
      "executable_path": "/usr/sbin/nginx",
      "executable_sha256": "b70d8f5b7e1db2b6ad1b4d7cf7b2a0ef8e5bb9a731f4ef9876f66dbadf4c6bb9",
      "package_name": "nginx",
      "package_manager": "deb",
      "version_text": "1.24.0"
    }
  ]
}
```

adapter 输出：

- `SwEv`

归一规则：

- `sha256` 是 artifact 级强证据。
- `purl` 是 package 生态标识。
- `CPE` 是漏洞情报映射线索，不是内部主键。

### 6.10 制品验真输入 `IN-10`

payload 样式：

```json
{
  "artifact_verifications": [
    {
      "artifact_hint": {
        "sha256": "b70d8f5b7e1db2b6ad1b4d7cf7b2a0ef8e5bb9a731f4ef9876f66dbadf4c6bb9"
      },
      "expected_sha256": "b70d8f5b7e1db2b6ad1b4d7cf7b2a0ef8e5bb9a731f4ef9876f66dbadf4c6bb9",
      "observed_sha256": "b70d8f5b7e1db2b6ad1b4d7cf7b2a0ef8e5bb9a731f4ef9876f66dbadf4c6bb9",
      "signature_status": "valid",
      "trusted_source": true
    }
  ]
}
```

adapter 输出：

- `ArtifactVerifyCand`

### 6.11 漏洞情报输入 `IN-11`

payload 样式：

```json
{
  "advisories": [
    {
      "advisory_source": "osv",
      "advisory_id": "OSV-2026-0001",
      "aliases": ["CVE-2026-10001"],
      "affected_packages": [],
      "published_at": "2026-04-25T18:00:00Z"
    }
  ]
}
```

adapter 输出：

- `VulnAdvisoryRaw`
- `AffectedRangeCand`
- `FindingCand`

### 6.12 BUG / 错误信号输入 `IN-12`

payload 样式：

```json
{
  "bug_observations": [
    {
      "error_signature": "java.lang.NullPointerException:CheckoutController:submitOrder:v2",
      "service_external_ref": "svc-checkout-api",
      "sample_count": 27,
      "first_observed_at": "2026-04-26T02:25:18Z",
      "last_observed_at": "2026-04-26T02:29:55Z"
    }
  ]
}
```

adapter 输出：

- `BugEv`
- `BugObs`
- `BugFindingCand`

约束：

- 单条错误日志不能直接创建 `SoftwareBug`。
- 需要错误签名、窗口聚合、版本或制品归因。

### 6.13 主体与组织输入 `IN-13`

payload 样式：

```json
{
  "users": [],
  "groups": [],
  "memberships": []
}
```

adapter 输出：

- `SubjectCand`
- `SubjectMemberCand`

### 6.14 责任与值班输入 `IN-14`

payload 样式：

```json
{
  "rotations": [],
  "alert_routes": [],
  "responsibility_assignments": []
}
```

adapter 输出：

- `RespCand`
- `OncallCand`

来源优先级：

1. `manual`
2. `cmdb_sync`
3. `oncall_sync`
4. `rule_derived`

### 6.15 安全与风险输入 `IN-15`

payload 样式：

```json
{
  "events": [
    {
      "event_id": "edr-998812",
      "event_type": "malicious_script",
      "severity": "high",
      "host": {
        "machine_id": "office-machine-002"
      },
      "artifact": {
        "script_sha256": "91f3d8f4ce13e8d947cb874fba2bf8f6f88f7c2b9d9c51c56699f32e2a8e7b91"
      }
    }
  ]
}
```

adapter 输出：

- `ThreatEv`
- `RiskCand`

### 6.16 人工修正输入 `IN-16`

payload 样式：

```json
{
  "corrections": [
    {
      "correction_id": "corr-20260426-001",
      "target_kind": "responsibility_assignment",
      "target_external_ref": "asset-linux-002",
      "operation": "upsert",
      "reason": "approved ownership correction",
      "approved_by": "alice@example.com"
    }
  ]
}
```

adapter 输出：

- `CorrectionCand`
- audit event

---

## 7. 示例文件索引

Target 示例按 `fixtures/external-input/target` 组织：

| 示例 | 覆盖分类 |
| --- | --- |
| [`edge-discovery-snapshot.json`](../../fixtures/external-input/target/edge-discovery-snapshot.json) | `IN-01`、`IN-03`、`IN-07`、`IN-09` |
| [`cmdb-catalog-snapshot.json`](../../fixtures/external-input/target/cmdb-catalog-snapshot.json) | `IN-01`、`IN-05`、`IN-14` |
| [`iam-directory-snapshot.json`](../../fixtures/external-input/target/iam-directory-snapshot.json) | `IN-13` |
| [`iam-directory-incremental.json`](../../fixtures/external-input/target/iam-directory-incremental.json) | `IN-13` |
| [`k8s-inventory-snapshot.json`](../../fixtures/external-input/target/k8s-inventory-snapshot.json) | `IN-04`、`IN-06`、`IN-07` |
| [`sw-evidence-snapshot.json`](../../fixtures/external-input/target/sw-evidence-snapshot.json) | `IN-09` |
| [`artifact-verification-snapshot.json`](../../fixtures/external-input/target/artifact-verification-snapshot.json) | `IN-10` |
| [`telemetry-dependency-window.json`](../../fixtures/external-input/target/telemetry-dependency-window.json) | `IN-08` |
| [`vuln-advisory-snapshot.json`](../../fixtures/external-input/target/vuln-advisory-snapshot.json) | `IN-11` |
| [`bug-signal-window.json`](../../fixtures/external-input/target/bug-signal-window.json) | `IN-12` |
| [`security-risk-snapshot.json`](../../fixtures/external-input/target/security-risk-snapshot.json) | `IN-15` |
| [`oncall-schedule-snapshot.json`](../../fixtures/external-input/target/oncall-schedule-snapshot.json) | `IN-14` |
| [`manual-host-responsibility-snapshot.json`](../../fixtures/external-input/target/manual-host-responsibility-snapshot.json) | `IN-14` |
| [`correction-host-owner.json`](../../fixtures/external-input/target/correction-host-owner.json) | `IN-16` |

---

## 8. 第一版验收标准

输入规范第一版验收标准：

- 每个模型文档涉及的输入至少能落到一个 `IN-*` 分类。
- 每个 `IN-*` 分类有明确 raw input 样式和 adapter 输出。
- 外部输入示例不包含中心内部 UUID。
- 时间、单位、枚举、外部标识命名统一。
- P0 normalized file ingest 与真实 external input 有清晰边界。
- 新增 connector 前必须先声明其 `schema_version`、payload 样式和 adapter 输出。
