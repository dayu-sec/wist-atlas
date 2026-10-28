# wist-atlas 拓扑可视化前端架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 可视化前端在当前阶段的最小可落地架构。

本文档重点解决两个问题：

- 前后端能否独立演进开发
- 双方应基于什么稳定边界对齐

这份文档不追求一次定义最终态全量拓扑 UI，而是优先固定：

- 第一版前端的职责与非目标
- 前后端并行开发所依赖的最小契约
- 第一版视图范围
- 前端内部模块与数据流
- 后续如何从最小视图平滑扩展

相关文档：

- [`../architecture/system-architecture.md`](../architecture/system-architecture.md)
- [`../architecture/query-and-read-model-architecture.md`](../architecture/query-and-read-model-architecture.md)
- [`../architecture/unified-model-overview.md`](../architecture/unified-model-overview.md)
- [`../architecture/scenario-and-scope-model.md`](../architecture/scenario-and-scope-model.md)
- [`./api-contract.md`](./api-contract.md)

---

## 2. 核心结论

当前阶段固定以下结论：

- 可视化前端可以独立于真实后端实现并行开发
- 并行开发的对齐基线不是 Rust domain 全集，而是单独的可视化 DTO 契约
- 第一版前端只覆盖 `host + network + responsibility` 的单图视图
- 第一版只做读，不做任何写入
- 第一版允许前端先消费 fixture 或 mock API，再切换到真实 API
- 场景差异体现在查询范围和视图内容，不体现在前端定义多套底层模型

一句话说：

- 前后端基于稳定的 `Visualization DTO` 对齐，而不是互相耦合实现细节

---

## 3. 当前阶段边界

### 3.1 场景边界

第一版前端优先服务以下场景：

- 家庭
- 中小企业早期场景（云和办公电脑）

这与整体 roadmap 保持一致：

- 先把 `host + 基础网络 + 基础地址 + 最小责任关系` 跑通
- 不默认引入中型企业及以上场景需要的复杂对象

### 3.2 对象边界

第一版前端只要求稳定支持以下对象：

- `HostInventory`
- `NetworkSegment`
- `Subject`

第一版图关系只要求稳定支持以下边：

- `HostNetAssoc`
- `ResponsibilityAssignment`

说明：

- `HostNetAssoc` 在可视化里是边，不是节点
- `ResponsibilityAssignment` 在可视化里是边，不是节点
- `NetworkDomain` 可作为后续增强对象，但不是首版强依赖
- `ServiceEntity`、`PodInventory`、`ClusterInventory`、`WorkloadEntity`、`DepEdge`、`ServiceInstance` 不是第一版前端前置条件

### 3.3 非目标

第一版前端不做：

- 五层全量统一图
- 服务依赖视图
- 影响传播视图
- dashboard 统计大盘
- 实时推送
- 图布局服务端计算
- 写入、修正、审批等治理操作

---

## 4. 系统边界

```text
Browser
  -> topology-visualization (React + TypeScript SPA)
  -> 读取 Visualization DTO
  -> 渲染 graph / detail / filter

HTTP JSON

wist-atlas HTTP 单体入口
  -> /api/topology/host/{id}
  -> /api/topology/network/{id}    (可选增强)
  -> /api/__mock/topology/host     (开发联调可选)
  -> static frontend assets        (部署方式可后定)

Storage / Query
  -> atlas-api query service
  -> atlas-storage
  -> PostgreSQL / mock backend
```

前端职责：

- 请求查询接口或 mock 接口
- 将 `TopologyGraph` 转换为 Cytoscape elements
- 管理筛选、搜索、选中、高亮等 UI 状态
- 展示节点详情

后端职责：

- 负责对象一致性和关系语义
- 输出满足契约的 `TopologyGraph`
- 保证边端点有效、ID 唯一、对象语义稳定

---

## 5. 对齐基线

前后端独立开发时，必须基于以下 5 类对齐物：

### 5.1 场景边界

首版只面向：

- 家庭
- 中小企业早期

### 5.2 首版对象范围

首版只要求：

- host 节点
- network segment 节点
- subject 节点
- host-network 边
- responsibility 边

### 5.3 可视化 DTO

前后端对齐的核心是 `Visualization DTO`，不是 Rust domain 全集。

### 5.4 固定 fixture

至少维护 2 份固定示例数据：

- `host + network` 图
- `host + network + responsibility` 图

前端可直接基于 fixture 开发，后端可基于同一 fixture 做 contract test。

### 5.5 验收问题

第一版至少能回答：

- 一台主机属于哪些网段
- 一台主机当前有哪些地址关联
- 谁负责这台主机

---

## 6. 技术选型

| 层面 | 选择 | 理由 |
|------|------|------|
| 语言 | TypeScript | strict 模式便于约束 DTO |
| 框架 | React | 生态成熟 |
| 构建 | Vite | 开发迭代快 |
| 图渲染 | Cytoscape.js | 原生节点/边模型适合拓扑图 |
| HTTP | `fetch` + typed wrapper | 足够支撑当前阶段 |
| 状态管理 | `useReducer` | 当前状态复杂度有限 |
| 测试 | Vitest + React Testing Library | 与 Vite 同生态 |

说明：

- 当前阶段不需要为了可视化引入新的 Rust 服务
- 当前阶段不需要为了状态管理引入 Redux/Zustand
- 当前阶段不需要为了类型生成引入复杂代码生成链

---

## 7. 类型体系

### 7.1 设计原则

- 前端不复刻 Rust domain 全集
- 前端只依赖可视化 DTO
- Rust domain 可演进，但只要 DTO 不破坏，前端即可独立演进
- DTO 的扩展应采用向后兼容方式

### 7.2 第一版 DTO

```typescript
type LayerKind =
  | 'resource'
  | 'governance';

type NodeKind =
  | 'HostInventory'
  | 'NetworkSegment'
  | 'Subject';

type EdgeKind =
  | 'host_network_assoc'
  | 'responsibility_assignment';

type TopologyNode = {
  id: string;               // 图内唯一 ID
  objectKind: NodeKind;     // 对应后端对象类型
  objectId: string;         // 对应后端对象 UUID
  layer: LayerKind;
  label: string;
  properties: Record<string, unknown>;
};

type TopologyEdge = {
  id: string;               // 图内唯一 ID
  edgeKind: EdgeKind;
  source: string;
  target: string;
  label?: string;
  properties?: Record<string, unknown>;
};

type TopologyGraph = {
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  metadata?: {
    queryTime: string;
    tenantId?: string;
    focusObjectKind?: NodeKind;
    focusObjectId?: string;
    truncated?: boolean;
  };
};

type ApiResponse<T> =
  | { status: 'ok'; data: T }
  | { status: 'error'; code: string; message: string };

type HostProcessTopologyGraph = {
  nodes: HostProcessTopologyNode[];
  edges: HostProcessTopologyEdge[];
  metadata?: {
    queryTime: string;
    focusObjectKind?: 'HostInventory';
    focusObjectId?: string;
    truncated?: boolean;
  };
};
```

### 7.3 为什么不直接复刻 Rust domain

原因：

- Rust domain 当前仍在扩展
- 前端不需要理解所有内部对象才能渲染第一版图
- 直接镜像 domain 会把后端未落地对象和未来对象提前暴露给前端
- read model / visualization model 的稳定性通常高于底层 domain 细节

补充边界：

- `TopologyGraph` 继续用于基础 topology 关系图
- `HostProcessTopologyGraph` 用于主机进程专视图
- `ProcessSummary` / `ProcessGroup` 只属于 host-process 专图，不应塞回通用 `TopologyGraph`

---

## 8. 组件结构

第一版只做一个主视图：

```text
App
├── Header
│   ├── Title
│   ├── SearchInput
│   └── RefreshButton
├── GraphView
│   ├── FilterBar
│   │   ├── LayerToggle
│   │   └── LayoutSelector
│   ├── TopologyCanvas
│   ├── NodeDetailPanel
│   └── LayerLegend
└── StatusBar
```

说明：

- `DashboardView`
- `DependencyView`
- `ImpactView`

都不属于当前第一版前端前置范围。

---

## 9. 数据流

```text
User Action
  -> dispatch(action)
  -> fetch graph
  -> validate ApiResponse<TopologyGraph | HostProcessTopologyGraph>
  -> graph to cytoscape elements
  -> render / update selection / update filters
```

建议状态结构：

```typescript
type AppState = {
  graph: TopologyGraph | HostProcessTopologyGraph | null;
  loading: boolean;
  error: string | null;
  selectedNodeId: string | null;
  layerVisibility: Record<LayerKind, boolean>;
  searchQuery: string;
};
```

说明：

- 当前不需要多视图路由
- 当前不需要把 Cytoscape instance 放进 React state

---

## 10. API Client

建议前端 API client 只暴露当前阶段真正稳定的方法：

```typescript
type TopologyApi = {
  getHostTopology(hostId: string): Promise<ApiResponse<TopologyGraph>>;
  getNetworkTopology?(networkId: string): Promise<ApiResponse<TopologyGraph>>;
  getMockHostTopology?(): Promise<ApiResponse<TopologyGraph>>;
  getHostProcessTopology?(hostId: string): Promise<ApiResponse<HostProcessTopologyGraph>>;
};
```

说明：

- `service topology`
- `dependency explain`
- `impact`
- `dashboard summary`

都不应作为首版前端 client 的强依赖。

---

## 11. 布局策略

当前阶段只需要两类布局：

| 场景 | 布局 |
|------|------|
| `host -> network / subject` 关系图 | `dagre` |
| 小规模探索图 | `cose-bilkent` |

建议默认：

- 首次渲染用 `dagre`
- 用户可手动切换 `cose-bilkent`

---

## 12. 独立开发模式

### 12.1 前端先行

前端可先基于：

- 本地 fixture
- `GET /api/__mock/topology/host`

完成以下工作：

- Cytoscape 渲染
- 节点详情面板
- 搜索与高亮
- 图层过滤
- 布局切换

### 12.2 后端并行

后端可独立推进：

- `HostTopologyView` 到 `TopologyGraph` 的 adapter
- HTTP handler
- mock backend / postgres backend
- contract test

### 12.3 联调切换

联调时只替换数据源：

- fixture -> mock API -> real API

只要返回的 DTO 保持稳定，前端不应因为后端内部实现变化而被阻塞。

---

## 13. 部署约束

前端已独立到 `wist-atlas-web` 仓，与 API 同域部署：

- `wist-atlas-web` 自行构建静态资源，并由镜像内的 nginx 托管
- nginx 把 `/api` 反代到 `wist-atlas`，浏览器只看到一个入口
- `wist-atlas` 只提供 HTTP API，不再托管前端静态资源
- 开发态由 Vite 把 `/api` 代理到本地后端
- 完整部署编排在 `wist-atlas-stack`

这样前端可以独立构建、独立发版，API 契约是两者之间唯一的耦合面。

---

## 14. 后续扩展顺序

在第一版稳定后，建议按以下顺序扩展：

1. `NetworkDomain`
2. `ServiceEntity`
3. `ServiceInstance`
4. `Cluster / Namespace / Workload / Pod`
5. `DepObs / DepEdge`
6. `Dashboard`
7. `Impact / Explain`

扩展原则：

- 新对象先进入 DTO，再进入 UI
- 不能反向要求前端先支持后端尚未稳定的对象全集
- 场景升级通过增加对象和视图完成，不通过重做底层契约完成

---

## 15. 当前建议

当前建议固定为：

- 第一版前端目标是最小可视化闭环，不是最终态全景平台
- 前后端并行开发必须以 `Visualization DTO + fixture + contract test` 为基础
- 第一版只做 `host + network + responsibility` 单图视图
- 未来扩展通过 DTO 兼容演进，不通过前端镜像 Rust domain 全集演进
