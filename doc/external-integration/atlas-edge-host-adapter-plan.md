# atlas-edge host adapter 工作思路

## 目标

把 `asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl` 中的 `atlas.in.edge.v1` host 记录接入 `wist-atlas`，先打通最小主机导入链路。

当前目标只覆盖：

- `schema = atlas.in.edge.v1`
- `payload` 为单条 host 发现事实
- 通过现有 monolith/file ingest 链路落成 `HostInventory`

当前明确不做：

- process / container 的正式 adapter
- `atlas.in.telemetry.v1` 的 runtime/telemetry materialize
- 多来源 identity resolution 扩展
- 外部 staged payload 持久化与 replay

## 当前现状

当前 `wist-atlas` 已有两层能力：

1. 接收 `AtlasInputEnvelope`
2. 接收 normalized batch import payload

但缺少一层：

- `atlas.in.edge.v1` -> normalized payload / candidates 的 adapter

也就是说，`dayu-edge.jsonl` 已经是 atlas 外部输入层，但还不能直接变成 `HostCandidate`。

## 分阶段做法

### 阶段 1：host-only adapter

先只支持 `payload.host_name/machine_id/external_ref` 这一类 host 记录。

输入：

```json
{
  "schema": "atlas.in.edge.v1",
  "source": { ... },
  "collect": { ... },
  "payload": {
    "host_name": "local-host",
    "machine_id": "hostname:local-host",
    "external_ref": "hostname:local-host"
  }
}
```

输出为现有 normalized payload：

```json
{
  "hosts": [
    {
      "host_name": "local-host",
      "machine_id": "hostname:local-host",
      "external_ref": "hostname:local-host"
    }
  ]
}
```

这样可以直接复用当前：

- `atlas-app`
- `TopologyIngestService`
- `extract_host_candidates`
- `materialize_host_network` 之前的 host materialize 流程

### 阶段 2：host + network

当 `atlas-edge` payload 开始携带 interface / ip / cidr 后，再补：

- `interfaces[]` / `network_interfaces[]`
- 映射到 normalized `ips[]` / `network_segments[]`

这样才能让 monolith summary 中的 network 相关字段稳定成立。

### 阶段 3：process

process 不应直接塞进当前 host-only normalized file ingest。

更合理的是新增单独 adapter 输出：

- `ProcessRuntimeState` 候选
- `SoftwareEvidence`
- runtime binding evidence

这一步应放在 host 锚点稳定之后。

## 最小代码策略

为了降低改动面，第一步不改 `atlas-api` / `atlas-domain` 的主抽象，先在 `atlas-app` 入口增加轻量适配：

1. 识别输入是否为 `atlas.in.edge.v1`
2. 如果是 host payload，则转换为 normalized payload
3. 再复用现有 `IngestEnvelope { source_kind = BatchImport }` 流程

这样做的优点：

- 改动小
- 容易验证
- 不会过早把 adapter 抽象固化错位

代价：

- 仍是 monolith 入口级 adapter
- 不是最终形态

这个代价当前可以接受，因为我们先要验证 host 能否稳定进入 `HostInventory`。

## 验收标准

第一版通过即可：

1. `atlas-app -- file <atlas-edge-host.json>` 可以运行
2. host 记录能 materialize 成 `HostInventory`
3. 如果没有网络字段，不要求 network summary 成功
4. 输出和错误信息要清晰区分“host 已导入”和“network 尚未提供”

## 下一步实现

按下面顺序做：

1. 在 `atlas-app` 增加 `atlas.in.edge.v1` host adapter
2. 准备最小 host fixture
3. 跑现有测试并补 host-only 验证
4. 再决定是否继续补 network 适配
