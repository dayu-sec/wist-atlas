# wist-atlas Rust 端可视化对接规范

## 1. 文档目的

本文档定义 Rust 端在当前阶段为了支撑可视化前端所需要提供的最小契约。

本文档只定义：

- 当前阶段前端真正依赖什么 API
- 请求和响应应采用什么格式
- Rust 端必须保证哪些一致性约束
- 如何支持前后端独立开发

本文档不定义：

- 前端内部实现细节
- Rust 内部 crate 组织方式
- 最终态全量拓扑接口

相关文档：

- [`./architecture.md`](./architecture.md)
- [`../architecture/scenario-and-scope-model.md`](../architecture/scenario-and-scope-model.md)
- [`../architecture/unified-model-overview.md`](../architecture/unified-model-overview.md)

---

## 2. 当前阶段结论

当前阶段固定以下结论：

- Rust 端先提供最小只读图查询能力
- 当前阶段 API 契约以 `Visualization DTO` 为准，不以 Rust domain 全集为准
- 第一版只要求支撑 `host + network + responsibility` 可视化
- 真实查询接口未完全落地前，可先提供 mock 接口
- 部署上建议同域，但静态文件由哪个 crate 提供暂不在本文档写死

一句话说：

- Rust 端对前端承诺的是稳定图 DTO，而不是内部模型细节

---

## 3. 第一版 API 范围

### 3.1 V1 必需端点

| 方法 | 路径 | 用途 |
|------|------|------|
| `GET` | `/api/topology/host/{id}` | 返回一台主机为中心的可视化子图 |

### 3.2 V1 可选增强

| 方法 | 路径 | 用途 |
|------|------|------|
| `GET` | `/api/topology/network/{id}` | 返回某个网络段为中心的可视化子图 |
| `GET` | `/api/__mock/topology/host` | 返回固定 mock 图，用于前后端并行开发 |

### 3.3 非 V1 范围

以下端点不属于当前第一版前置能力：

- `/api/topology/service/{id}`
- `/api/topology/graph`
- `/api/dashboard/summary`
- `/api/topology/expand/{nodeId}`
- `/api/explain/dependency`
- `/api/impact/from/{nodeId}`

这些能力可以在后续阶段增加，但不应作为当前前后端对齐基线。

---

## 4. 通用响应格式

所有接口统一返回：

```json
{
  "status": "ok",
  "data": { "...": "..." }
}
```

或：

```json
{
  "status": "error",
  "code": "NOT_FOUND",
  "message": "human-readable description"
}
```

错误码第一版只要求：

| code | 含义 |
|------|------|
| `NOT_FOUND` | 请求对象不存在 |
| `INVALID_PARAM` | 参数非法 |
| `INTERNAL` | 服务端内部错误 |

当前阶段不需要过早设计大量错误码。

---

## 5. 可视化 DTO

### 5.1 `LayerKind`

当前阶段只需要：

```json
"resource"
```

或：

```json
"governance"
```

### 5.2 `NodeKind`

当前阶段只允许：

```text
HostInventory
NetworkSegment
Subject
```

### 5.3 `EdgeKind`

当前阶段只允许：

```text
host_network_assoc
responsibility_assignment
```

### 5.4 `TopologyNode`

```json
{
  "id": "node-host-6f4d",
  "objectKind": "HostInventory",
  "objectId": "6f4dc4f0-bca1-4f5f-a5a8-0dfc0f8fdc18",
  "layer": "resource",
  "label": "demo-node",
  "properties": {
    "hostName": "demo-node",
    "machineId": "demo-machine-01"
  }
}
```

字段说明：

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `id` | string | 是 | 图内唯一元素 ID，不要求等于对象 UUID |
| `objectKind` | string | 是 | 后端对象类型 |
| `objectId` | string | 是 | 后端对象 UUID |
| `layer` | string | 是 | 当前阶段只允许 `resource` 或 `governance` |
| `label` | string | 是 | 节点展示名称 |
| `properties` | object | 是 | 节点详情属性，允许空对象 |

约束：

- `id` 用于图渲染和边引用
- `objectId` 用于详情跳转、focus 和后续查询
- 前端不能假设 `id == objectId`

### 5.5 `TopologyEdge`

```json
{
  "id": "edge-hostnet-1",
  "edgeKind": "host_network_assoc",
  "source": "node-host-6f4d",
  "target": "node-net-12ab",
  "label": "10.42.0.12",
  "properties": {
    "ipAddr": "10.42.0.12",
    "ifaceName": "eth0"
  }
}
```

字段说明：

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `id` | string | 是 | 边唯一 ID |
| `edgeKind` | string | 是 | 当前阶段只允许两类边 |
| `source` | string | 是 | 源节点 `id` |
| `target` | string | 是 | 目标节点 `id` |
| `label` | string | 否 | 图上展示文本 |
| `properties` | object | 否 | 边详情属性 |

约束：

- `HostNetAssoc` 是边
- `ResponsibilityAssignment` 是边
- 它们不应在当前阶段被建模成节点

### 5.6 `TopologyGraph`

```json
{
  "status": "ok",
  "data": {
    "nodes": [],
    "edges": [],
    "metadata": {
      "queryTime": "2026-04-25T10:30:00Z",
      "tenantId": "optional",
      "focusObjectKind": "HostInventory",
      "focusObjectId": "6f4dc4f0-bca1-4f5f-a5a8-0dfc0f8fdc18",
      "truncated": false
    }
  }
}
```

`metadata` 字段建议包括：

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `queryTime` | string | 是 | 查询时间 |
| `tenantId` | string | 否 | 多租户场景可用 |
| `focusObjectKind` | string | 否 | 当前图的中心对象类型 |
| `focusObjectId` | string | 否 | 当前图的中心对象 ID |
| `truncated` | bool | 否 | 若后续引入 limit，可指示是否截断 |

### 5.7 `HostProcessTopologyGraph`

该契约用于主机进程视角的专用图，不等同于通用 `TopologyGraph`。

它面向的问题是：

- 某台主机当前有哪些进程
- 这些进程按程序归并后是什么分布
- 每类程序的实例数、状态摘要和 RSS 汇总如何

它不是 source-of-truth 对象模型，而是 host-process 场景下的 derived visualization contract。

```json
{
  "status": "ok",
  "data": {
    "nodes": [
      {
        "id": "host:...",
        "objectKind": "HostInventory",
        "objectId": "...",
        "layer": "resource",
        "label": "local-host",
        "properties": {}
      },
      {
        "id": "process-summary:host:...",
        "objectKind": "ProcessSummary",
        "objectId": "...",
        "layer": "resource",
        "label": "processes: 780",
        "properties": {
          "totalProcesses": 780,
          "totalPrograms": 523,
          "topPrograms": "<defunct> x79, distnoted x20"
        }
      },
      {
        "id": "process-group:host:...:/usr/sbin/distnoted",
        "objectKind": "ProcessGroup",
        "objectId": "...",
        "layer": "resource",
        "label": "distnoted x20",
        "properties": {
          "executable": "/usr/sbin/distnoted",
          "processCount": 20,
          "totalMemoryRssKiB": 0,
          "dominantState": "-",
          "states": "-:20"
        }
      },
      {
        "id": "process:...",
        "objectKind": "ProcessRuntime",
        "objectId": "...",
        "layer": "resource",
        "label": "distnoted (121)",
        "properties": {
          "pid": 121,
          "executable": "/usr/sbin/distnoted"
        }
      }
    ],
    "edges": [],
    "metadata": {
      "queryTime": "2026-05-13T10:30:00Z",
      "focusObjectKind": "HostInventory",
      "focusObjectId": "..."
    }
  }
}
```

补充对象种类：

- `ProcessSummary`
- `ProcessGroup`
- `ProcessRuntime`

补充语义约束：

- `ProcessSummary` 表示主机级进程概览节点
- `ProcessGroup` 表示按 `executable` 归并后的程序组节点
- `ProcessRuntime` 表示具体进程实例节点
- `ProcessSummary` 和 `ProcessGroup` 是 derived visualization node，不是主库基础对象

---

## 6. `GET /api/topology/host/{id}`

### 6.1 语义

以某台主机为中心，返回第一版可视化需要的最小子图。

图中允许出现：

- 一个 `HostInventory` 节点
- 零到多个 `NetworkSegment` 节点
- 零到多个 `Subject` 节点
- `host_network_assoc` 边
- `responsibility_assignment` 边

### 6.2 请求

```text
GET /api/topology/host/{id}
```

其中：

- `{id}` 是主机对象 UUID

当前阶段不定义复杂查询参数。

### 6.3 成功响应示例

```json
{
  "status": "ok",
  "data": {
    "nodes": [
      {
        "id": "node-host-1",
        "objectKind": "HostInventory",
        "objectId": "11111111-1111-1111-1111-111111111111",
        "layer": "resource",
        "label": "demo-node",
        "properties": {
          "hostName": "demo-node",
          "machineId": "demo-machine-01"
        }
      },
      {
        "id": "node-net-1",
        "objectKind": "NetworkSegment",
        "objectId": "22222222-2222-2222-2222-222222222222",
        "layer": "resource",
        "label": "10.42.0.0/24",
        "properties": {
          "cidr": "10.42.0.0/24",
          "gatewayIp": "10.42.0.1"
        }
      },
      {
        "id": "node-subject-1",
        "objectKind": "Subject",
        "objectId": "33333333-3333-3333-3333-333333333333",
        "layer": "governance",
        "label": "alice",
        "properties": {
          "email": "alice@example.com"
        }
      }
    ],
    "edges": [
      {
        "id": "edge-hostnet-1",
        "edgeKind": "host_network_assoc",
        "source": "node-host-1",
        "target": "node-net-1",
        "label": "10.42.0.12",
        "properties": {
          "ipAddr": "10.42.0.12",
          "ifaceName": "eth0"
        }
      },
      {
        "id": "edge-owner-1",
        "edgeKind": "responsibility_assignment",
        "source": "node-subject-1",
        "target": "node-host-1",
        "label": "Owner"
      }
    ],
    "metadata": {
      "queryTime": "2026-04-25T10:30:00Z",
      "focusObjectKind": "HostInventory",
      "focusObjectId": "11111111-1111-1111-1111-111111111111"
    }
  }
}
```

### 6.4 错误响应

若主机不存在：

```json
{
  "status": "error",
  "code": "NOT_FOUND",
  "message": "host not found"
}
```

---

## 7. `GET /api/topology/network/{id}`（可选增强）

这个接口不是第一版强依赖。

若实现，其语义应为：

- 以某个 `NetworkSegment` 为中心
- 返回与其关联的 host 与 subject 信息

图对象范围仍然只能是：

- `HostInventory`
- `NetworkSegment`
- `Subject`
- `host_network_assoc`
- `responsibility_assignment`

不应借该接口提前引入未稳定的运行态和依赖对象。

---

## 8. Mock API

为支持前后端独立开发，建议 Rust 端提供：

```text
GET /api/__mock/topology/host
```

返回一份固定 `TopologyGraph`。

用途：

- 前端先独立完成渲染与交互
- 后端未接好真实 query handler 前，也能保持联调链路稳定

约束：

- mock 返回格式必须和真实 API 一致
- mock 数据建议来源于固定 fixture，而不是随机生成

---

## 9. Rust 端内部适配原则

Rust 端内部可自由演进，但对前端应收敛到单一适配层：

```text
read model / query service
  -> visualization adapter
  -> TopologyGraph DTO
  -> HTTP JSON
```

当前建议：

- 先从已有 `HostTopologyView` 做 adapter
- 再从 `NetworkTopologyView` 做 adapter
- 不要求前端直接消费 Rust 内部 read model JSON
- host-process 场景单独使用 `HostProcessTopologyGraph`
- 不把 `ProcessSummary` / `ProcessGroup` 混入通用 `TopologyGraph`

原因：

- `HostTopologyView` 是后端内部读模型
- `TopologyGraph` 是前后端公共契约
- `HostProcessTopologyGraph` 是 host-process 专用契约
- `ProcessSummary` / `ProcessGroup` 是展示导向聚合节点，不是通用 topology object
- 两者职责不同，不应混为一层

---

## 10. 一致性约束

以下约束必须由 Rust 端保证：

1. `nodes[].id` 在同次响应中唯一
2. `edges[].id` 在同次响应中唯一
3. 每条边的 `source` 和 `target` 都必须能在 `nodes[]` 中找到
4. `objectId` 必须是稳定后端对象 ID，而不是临时显示名称
5. 当前阶段 `layer` 只能取：
   - `resource`
   - `governance`
6. 当前阶段 `objectKind` 只能取：
   - `HostInventory`
   - `NetworkSegment`
   - `Subject`
7. 当前阶段 `edgeKind` 只能取：
   - `host_network_assoc`
   - `responsibility_assignment`

这些约束不应推给前端在运行时兜底。

---

## 11. 部署约束

前端已独立到 `wist-atlas-web` 仓，部署约束固定为：

- 前端和 API 同域：`wist-atlas-web` 的 nginx 把 `/api` 反代到 `wist-atlas`
- 前端静态资源由 `wist-atlas-web` 自己构建和托管，不在后端仓
- 本文档定义的 API 契约是两者之间唯一的耦合面

编排细节由 `wist-atlas-stack` 负责，不属于前后端契约。

---

## 12. Contract Test 建议

为了保证前后端独立演进时不漂移，建议增加 contract test：

- 后端：
  - 以固定 fixture 生成 `TopologyGraph`
  - 校验节点/边数量与关键字段
- 前端：
  - 用同一 fixture 渲染
  - 校验节点展示、边展示、详情展示

最少覆盖两个样例：

1. `host + network`
2. `host + network + responsibility`

---

## 13. 后续扩展原则

未来若扩展到：

- `ServiceEntity`
- `ServiceInstance`
- `PodInventory`
- `ClusterInventory`
- `DepObs / DepEdge`
- `Impact / Explain`

应遵循以下顺序：

1. 先扩 DTO
2. 再补 adapter
3. 再补 API
4. 再补前端 UI

不应反向要求前端先支持完整对象全集，或让前端直接跟随后端 domain 演化。

---

## 14. 当前建议

当前建议固定为：

- Rust 端第一版只承诺最小图查询能力
- 前后端通过 `Visualization DTO + fixture + contract test` 对齐
- `HostNetAssoc` 和 `ResponsibilityAssignment` 当前阶段都按边处理
- 真实 API 落地前，mock API 是允许且推荐的并行开发手段
