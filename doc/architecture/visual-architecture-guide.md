# wist-atlas 可视化架构指南

## 1. 文档目的

本文档把 `wist-atlas` 的关键设计整理成可传播的专业图示。每张图只配简短说明，详细论述见对应的架构文档。

## 2. 一页总览

```mermaid
flowchart LR
    subgraph Producers["事实来源"]
        Edge["Edge / Discovery<br/>host / process / pod / runtime"]
        Manual["Manual / Batch<br/>业务目录 / 责任关系"]
        Telemetry["Telemetry Summary<br/>trace / log / flow 摘要"]
        External["External Systems<br/>CMDB / LDAP / IAM / Oncall / Vuln"]
    end

    subgraph Topology["wist-atlas 中心侧"]
        Ingest["Ingest Gateway<br/>鉴权 / 幂等 / envelope"]
        Normalize["Normalization Engine<br/>candidate / resolver / materializer"]
        Store[("Catalog Store<br/>PostgreSQL source of truth")]
        ObjectStore[("Object Storage<br/>raw payload / snapshot")]
        Derive["Derived View Builder<br/>summary / graph / explain view"]
        Query["Query API<br/>catalog / topology / governance / explain"]
        Sync["External Sync Workers<br/>connector / cursor / staged payload"]
    end

    subgraph Consumers["消费方"]
        UI["控制中心 / UI"]
        Analytics["分析系统"]
        Viz["2D / 3D 可视化"]
        Governance["治理与审计"]
    end

    Edge --> Ingest
    Manual --> Ingest
    Telemetry --> Ingest
    External --> Sync --> Ingest
    Ingest --> ObjectStore
    Ingest --> Normalize --> Store
    Store --> Derive
    Store --> Query
    Derive --> Query
    Query --> UI
    Query --> Analytics
    Query --> Viz
    Query --> Governance
```

外部输入只是事实来源，不直接等于中心对象。`Normalization Engine` 是中心语义层，PostgreSQL 是 source of truth。

## 3. 系统边界图

```mermaid
flowchart TB
    subgraph Outside["wist-atlas 外部"]
        Agent["边缘 Agent / Discovery"]
        Control["控制中心<br/>审批 / 编译 / 下发 / 执行跟踪"]
        Cmdb["CMDB / LDAP / IAM / Oncall"]
        Obs["Telemetry / Log / Trace / Flow"]
        Visual["可视化系统"]
    end

    subgraph Atlas["wist-atlas"]
        Model["统一对象模型"]
        Relation["统一关系图谱"]
        Resolve["Identity Resolution"]
        Query["Query / Graph / Explain API"]
        Sync["External Sync"]
    end

    Agent -- "发现结果 / 运行事实 / 软件线索" --> Model
    Obs -- "摘要观测 / 依赖证据" --> Resolve
    Cmdb -- "组织 / 责任 / 外部身份" --> Sync
    Sync --> Resolve
    Resolve --> Model
    Model --> Relation
    Relation --> Query
    Query -- "拓扑 / 责任 / 风险 / explain" --> Control
    Query -- "稳定 read model" --> Visual

    Control -. "不属于 wist-atlas" .- Control
    Agent -. "不由中心侧执行 discovery" .- Agent
```

`wist-atlas` 不做边缘采集，不做控制面执行。可视化系统消费 read model，不重新定义领域模型。

## 4. 逻辑模块与职责

```mermaid
flowchart LR
    Ingest["Ingest Gateway"]
    Normalize["Normalization Engine"]
    Store["Catalog Store"]
    Derive["Derived View Builder"]
    Query["Query API"]
    Sync["External Sync Workers"]

    Ingest --> Normalize
    Normalize --> Store
    Store --> Derive
    Store --> Query
    Derive --> Query
    Sync --> Ingest

    subgraph IngestResp["Ingest Gateway 职责"]
        IG1["协议接入"]
        IG2["鉴权"]
        IG3["幂等检查"]
        IG4["IngestEnvelope"]
    end

    subgraph NormResp["Normalization Engine 职责"]
        NE1["candidate extraction"]
        NE2["identity resolution"]
        NE3["relation materialization"]
        NE4["evidence / explain"]
    end

    subgraph StoreResp["Catalog Store 职责"]
        CS1["主对象"]
        CS2["关系边"]
        CS3["运行态"]
        CS4["同步游标"]
    end

    Ingest -.-> IngestResp
    Normalize -.-> NormResp
    Store -.-> StoreResp
```

写路径负责事实归一和主对象落库，读路径负责投影成稳定视图。sync 是独立运行时能力，不绕过 normalization。

## 5. 主写路径 Pipeline

```mermaid
flowchart LR
    Raw["Raw Payload<br/>外部事实"]
    Envelope["IngestEnvelope<br/>source / tenant / observed_at / payload_ref"]
    Typed["Typed Raw Event<br/>schema 校验"]
    Candidate["Candidate / Evidence / Observation<br/>候选层"]
    Resolve["Identity Resolution<br/>认主机 / 服务 / 主体 / 软件"]
    Materialize["Materializer<br/>对象与关系落库"]
    SOT[("Source of Truth<br/>PostgreSQL")]
    Dead["Dead Letter / Unresolved<br/>不写正式关系"]

    Raw --> Envelope
    Envelope --> Typed
    Typed --> Candidate
    Candidate --> Resolve
    Resolve -- "matched / created" --> Materialize --> SOT
    Resolve -- "unresolved / conflicting" --> Dead
```

Candidate 不是中心主对象。Unresolved candidate 不允许硬写成正式关系。Resolver 必须保留来源、置信度和 explain 信息。

## 6. Identity Resolution 流程

```mermaid
sequenceDiagram
    participant P as Parser / Validator
    participant C as Candidate Extractor
    participant R as Identity Resolver
    participant L as ExternalIdentityLink
    participant S as Catalog Store
    participant Q as Conflict Queue

    P->>C: typed raw event
    C->>R: candidate + identifiers
    R->>L: query external mapping
    R->>S: match by strong / composite / weak identifiers

    alt high confidence match
        R->>S: reuse internal id
        R->>L: upsert mapping
    else no match
        R->>S: assign new internal id
        R->>L: create mapping
    else conflicting candidates
        R->>Q: record conflict and evidence
        R-->>C: unresolved / conflicting
    end
```

规则层次：强标识 (`machine_id`, `pod_uid`, `external_id`, `purl`) > 组合标识 (`cluster+namespace+kind+name`) > 弱标识（仅辅助判断）。

## 7. 统一模型分层图

```mermaid
flowchart TB
    subgraph Business["业务架构层"]
        BD["BusinessDomain"]
        SYS["SystemBoundary"]
        SUB["Subsystem"]
        SVC["ServiceEntity"]
    end

    subgraph Catalog["资源目录层"]
        HOST["HostInventory"]
        POD["PodInventory"]
        CLUSTER["ClusterInventory"]
        NS["NamespaceInventory"]
        WORKLOAD["WorkloadEntity"]
        SW["SoftwareEntity"]
    end

    subgraph Runtime["运行实例层"]
        HRS["HostRuntimeState"]
        PROC["ProcessRuntimeState"]
        INST["ServiceInstance"]
        CEP["ContainerRuntime"]
        EP["SvcEp / InstEp"]
    end

    subgraph Relation["关系图谱层"]
        PLACE["PodPlacement"]
        BIND["RuntimeBinding"]
        DEP["DepEdge / DepObs"]
        NET["HostNetAssoc / PodNetAssoc"]
        EVID["SoftwareEvidence"]
    end

    subgraph Governance["责任治理层"]
        SUBJECT["Subject"]
        ASSIGN["ResponsibilityAssignment"]
        LINK["ExternalIdentityLink"]
        CURSOR["ExternalSyncCursor"]
        FINDING["SoftwareVulnerabilityFinding"]
    end

    BD --> SYS --> SUB --> SVC
    SVC --> WORKLOAD
    CLUSTER --> NS --> WORKLOAD --> POD
    POD --> PLACE --> HOST
    HOST --> HRS
    SVC --> INST --> BIND
    BIND --> POD
    BIND --> PROC
    CEP --> POD
    PROC --> EVID --> SW --> FINDING
    SVC --> DEP
    HOST --> NET
    POD --> NET
    ASSIGN --> SUBJECT
    HOST --> ASSIGN
    SVC --> ASSIGN
    LINK --> SUBJECT
    CURSOR --> LINK
```

五层模型：业务架构层回答"业务和系统如何组织"，资源目录层回答"资源和服务是谁"，运行实例层回答"当前状态"，关系图谱层回答"对象间关系"，责任治理层回答"谁负责、受什么漏洞影响"。

## 8. 存储分层图

```mermaid
flowchart TB
    subgraph WritePath["写路径"]
        Normalize["Normalization Engine"]
        Materializer["Materializer"]
    end

    subgraph Storage["存储层"]
        PG[("PostgreSQL<br/>source of truth")]
        OBJ[("Object Storage<br/>raw payload / snapshot")]
        Cache[("Cache<br/>optional acceleration")]
    end

    subgraph PgLogical["PostgreSQL 逻辑分区"]
        Catalog["catalog<br/>业务 / 服务 / 主机 / Pod / 软件"]
        Runtime["runtime<br/>运行态 / binding / observation"]
        Governance["governance<br/>责任 / evidence / audit 摘要"]
        Sync["sync<br/>external link / cursor / job"]
    end

    Normalize --> Materializer
    Materializer --> PG
    Materializer --> OBJ
    PG --> Catalog
    PG --> Runtime
    PG --> Governance
    PG --> Sync
    PG --> Cache
```

主对象和关系对象进入 PostgreSQL，原始 payload 和大快照进入对象存储，缓存只做加速。

## 9. 读路径与 Read Model

```mermaid
flowchart LR
    SOT[("Source of Truth<br/>PostgreSQL")]
    Builder["Derived View Builder"]
    ObjectQuery["Object Query<br/>单对象 / 列表 / 轻过滤"]
    ReadModel["Derived Read Model<br/>overview / summary / effective responsibility"]
    Explain["Explain / Graph Query<br/>evidence chain / graph traversal"]
    API["Query API"]
    Consumer["UI / 控制中心 / 分析系统 / 可视化"]

    SOT --> ObjectQuery
    SOT --> Builder --> ReadModel
    SOT --> Explain
    ReadModel --> Explain
    ObjectQuery --> API
    ReadModel --> API
    Explain --> API
    API --> Consumer
```

简单对象查询可直读主库，复杂聚合与全景视图走 read model，explain 与 graph 查询独立分层，Query API 不直接暴露底表。

## 10. External Sync 流程

```mermaid
flowchart LR
    Source["External System<br/>CMDB / LDAP / IAM / Oncall / Vuln"]
    Connector["Connector<br/>auth / paging / rate limit"]
    Stage["Fetch & Stage<br/>raw payload / metadata"]
    Normalize["Normalize & Resolve<br/>external id -> internal id"]
    Persist["Persist<br/>upsert source of truth"]
    Cursor["Advance Cursor<br/>only after persist success"]
    Store[("PostgreSQL")]
    Replay["Replay<br/>from staged payload"]
    Fail["Failure<br/>isolate source / keep cursor"]

    Source --> Connector --> Stage --> Normalize --> Persist --> Store --> Cursor
    Stage --> Replay --> Normalize
    Connector -. failure .-> Fail
    Normalize -. conflict .-> Fail
    Persist -. failure .-> Fail
    Fail -. no cursor advance .-> Cursor
```

Cursor 只在主写成功后推进，staged payload 支持失败重放，一个源失败不阻塞其他源。

## 11. 第一版部署演进

```mermaid
flowchart TB
    subgraph Phase1["Phase 1：单体起步"]
        Mono["wist-atlas-server<br/>API + ingest + normalize + query"]
        PG1[("PostgreSQL")]
        OBJ1[("Object Storage")]
        Mono --> PG1
        Mono --> OBJ1
    end

    subgraph Phase2["Phase 2：单体 + Worker"]
        API2["wist-atlas-server"]
        Worker2["wist-atlas-worker<br/>batch normalize / derive / explain rebuild"]
        PG2[("PostgreSQL")]
        OBJ2[("Object Storage")]
        API2 --> PG2
        Worker2 --> PG2
        API2 --> OBJ2
        Worker2 --> OBJ2
    end

    subgraph Phase3["Phase 3：API / Worker / Sync 三分"]
        API3["wist-atlas-api"]
        Worker3["wist-atlas-worker"]
        Sync3["wist-atlas-sync"]
        PG3[("PostgreSQL")]
        OBJ3[("Object Storage")]
        API3 --> PG3
        Worker3 --> PG3
        Sync3 --> PG3
        Sync3 --> OBJ3
        Worker3 --> OBJ3
    end

    Phase1 --> Phase2 --> Phase3
```

第一版单体优先，逻辑边界先清楚，物理拆分按压力与隔离需求推进。

## 12. Crate 映射

```mermaid
flowchart LR
    Domain["atlas-domain<br/>领域对象 / contract / DTO"]
    Storage["atlas-storage<br/>repository / migration / Postgres"]
    API["atlas-api<br/>ingest / query / view / explain"]
    Sync["atlas-sync<br/>connector / runner / cursor"]
    App["atlas-app<br/>config / role / process entry"]

    Domain --> Storage
    Domain --> API
    Domain --> Sync
    Storage --> API
    Storage --> Sync
    API --> App
    Sync --> App
    Storage --> App
```

`atlas-domain` 是领域语义单一来源，`atlas-storage` 只做存储 contract 和实现，`atlas-api` 提供 ingest 与 query，`atlas-sync` 提供外部同步，`atlas-app` 只做装配。

## 13. 第一版开发路线

```mermaid
gantt
    title wist-atlas 第一版开发路线
    dateFormat  YYYY-MM-DD
    axisFormat  W%W

    section Foundation
    Phase 0 固定实现基线           :p0, 2026-04-20, 1w
    Phase 1 领域模型与主存储       :p1, after p0, 1w

    section Core Loop
    Phase 2 最小写路径闭环         :p2, after p1, 2w
    Phase 3 最小 Query API         :p3, after p2, 1w

    section Integration
    Phase 4 外部同步基础能力       :p4, after p3, 2w
    Phase 5 派生视图与治理扩展     :p5, after p4, 2w
```

先让模型和主库站稳，再打通 ingest 到 query 的最小闭环，sync 和 derived view 建立在稳定 source of truth 之上。

## 14. Web 可视化

本文档包含设计阶段的静态架构图。Web 端交互式拓扑可视化的前端架构设计，见 [`../frontend/architecture.md`](../frontend/architecture.md)。

## 15. 模型详细图解索引

以下图解分布在对应的模型与架构文档中，本文档只保留总览图。

### 构建与依赖

| 图 | 位置 |
|---|---|
| 图 A：模型构建阶段（Phase A→E） | [`unified-model-overview.md` §5.4](./unified-model-overview.md#54-实现顺序建议) |
| 图 B：模型硬依赖与集成依赖 | [`unified-model-overview.md` §5.3](./unified-model-overview.md#53-集成依赖关系) |
| 图 C：三条主干依赖关系 | [`unified-model-overview.md` §5.1](./unified-model-overview.md#51-三条主干) |

### 模型内实体关系

| 模型文档 | 图 | 位置 |
|---|---|---|
| `business-system-service-topology-model.md` | 业务架构层 ER 图 | [§5 对象模型](../model/business-system-service-topology-model.md) |
| `host-inventory-and-runtime-state.md` | 主机目录与运行态 ER 图 | [§5 对象模型](../model/host-inventory-and-runtime-state.md) |
| `host-pod-network-topology-model.md` | 网络拓扑 ER 图 | [§5 对象模型](../model/host-pod-network-topology-model.md) |
| `cluster-namespace-workload-topology-model.md` | 集群编排 ER 图 | [§5 对象模型](../model/cluster-namespace-workload-topology-model.md) |
| `runtime-binding-model.md` | 运行绑定关系图 | [§5 对象与关系模型](../model/runtime-binding-model.md) |
| `endpoint-and-dependency-observation-model.md` | 端点与依赖观测图 | [§5 对象模型](../model/endpoint-and-dependency-observation-model.md) |
| `software-normalization-and-vuln-enrichment.md` | 软件与漏洞 ER 图 | [§5 对象模型](../model/software-normalization-and-vuln-enrichment.md) |
| `host-process-software-vulnerability-graph.md` | 主机-进程-软件-漏洞链路图 | [§5 禁止事项](../model/host-process-software-vulnerability-graph.md) |
| `host-responsibility-and-maintainer-model.md` | 责任治理 ER 图 | [§5 角色定义](../model/host-responsibility-and-maintainer-model.md) |
| `public-vulnerability-source-ingestion.md` | 漏洞摄入流程图 | [§5 两条主路线](../model/public-vulnerability-source-ingestion.md) |
| `host-responsibility-sync-from-external-systems.md` | 外部同步流程图 | [§6 ExternalSyncCursor](../model/host-responsibility-sync-from-external-systems.md) |
