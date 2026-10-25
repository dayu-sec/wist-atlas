# 外部对接术语表

## 1. 文档目的

本文档统一 `wist-atlas` 与外部系统对接时使用的关键术语，重点覆盖 `warp-insight` discovery 对接链路。

本文档只定义外部系统需要理解的输入事实、来源标识和边界语义，不定义 atlas 内部 adapter / resolver / materializer 的中间对象、代码字段名或 pipeline 实现约束。

目标是避免以下混用：

- 把边缘 `resource` 当成 atlas 中心主对象
- 把 `target` 当成采集配置、endpoint 或服务实例
- 把 `snapshot` 当成中心资源目录
- 让外部来源构造 atlas 内部候选对象、证据对象或主键

相关文档：

- [`README.md`](./README.md)
- [`input-taxonomy-and-style.md`](./input-taxonomy-and-style.md)
- [`external-input-spec.md`](./external-input-spec.md)
- [`warp-insight-adapter-spec.md`](./warp-insight-adapter-spec.md)
- [`../glossary.md`](../glossary.md)
- [`warp-insight/doc/design/foundation/glossary.md`](../../../warp-insight/doc/design/foundation/glossary.md)

内部实现术语、adapter 输出短名和 lower_snake 字段名见 [`../internal/processing-glossary.md`](../internal/processing-glossary.md)，不属于外部接口契约。

---

## 2. 总体规则

统一规则：

- 外部来源术语必须保留来源作用域，例如 `warp-insight DiscoveredResource`。
- 外部输入只表达来源系统观察到或声明的事实，不表达 atlas 中心主对象。
- 同一个英文词跨层使用时必须带限定词，例如 `external resource fact`、`center resource catalog`。
- 外部协议对象保留原名；atlas 文档可提供短名，但不得把短名写回原始 payload。
- `Snapshot`、`Delta`、`Window` 等同步形态术语的细化规则见第 6 节。

禁止：

- 用 `resource` 裸词同时表示边缘资源、中心资源、采集目标。
- 用 `target` 裸词表示服务、实例、endpoint 或采集配置。
- 用 `id` 裸词表示内部主键或外部标识。
- 要求外部系统提交 atlas 内部 candidate、evidence、resolver 或 materializer 对象。
- 把外部协议字段名强行缩短后写入原始 payload；外部原名必须可回放。

短名规则：

- 外部协议对象保留原名，文档中可在首次出现时写“原名（短名）”。
- 短名只用于 atlas 文档阅读便利，不是外部 payload 字段名。
- 具体 adapter 输出短名和代码字段名只在内部文档中定义。

---

## 3. 对外核心术语

| 统一术语 | 短名 | 中文名 | 作用域 | 定义 | 不应混用 |
| --- | --- | --- | --- | --- | --- |
| `External Raw Input` | `RawInput` | 外部原始输入 | atlas 对接层 | 外部系统给 atlas 的结构化事实载荷 | atlas 内部候选对象 / 中心主表 |
| `External Source` | `ExtSource` | 外部来源 | atlas 对接层 | 产生输入的系统，例如 `warp-insight`、CMDB、IAM | adapter / connector 实例 |
| `Producer` | `Producer` | 生产者 | 外部来源侧 | 产生某份 payload 的 agent、job 或 connector | tenant / source system |
| `Resource Fact` | `ResFact` | 资源事实 | 对接事实层 | 外部系统观察到的资源属性、状态或生命周期事实，不是中心资源目录对象 | `HostInventory` / `ServiceEntity` |
| `Relation Fact` | `RelFact` | 关系事实 | 对接事实层 | 外部系统观察到的资源间关系事实，例如 host-process、container-pod、process-listens-on | final topology edge / ownership relation |
| `Target Evidence` | `TargetEv` | 目标证据 | 对接事实层 | 外部系统发现的候选采集目标线索，只证明“可采集/可观测” | endpoint / service instance / collection config |
| `Resource Fact Snapshot` | `ResFactSnap` | 资源事实快照 | 对接事实层 | 某来源在某时间形成的一组资源事实 | center catalog snapshot |
| `Source-of-truth Object` | `SotObject` | 主事实对象 | atlas 中心模型 | 经过 atlas 内部解析、归并、幂等写入的中心对象 | external resource fact |
| `Derived View` | `View` | 派生视图 | atlas 查询层 | 可重算的查询视图 | source-of-truth |

边界：

- 外部系统可以提交 `RawInput`、`ResFact`、`RelFact`、`TargetEv`。
- 外部系统不提交 atlas 内部候选、证据或观测对象。
- `SotObject` 和 `View` 只作为边界说明出现，不是外部输入结构。

---

## 4. warp-insight 术语对齐

| warp-insight 原名 | atlas 文档短名 | 中文名 | atlas 对应语义 | 处理规则 |
| --- | --- | --- | --- | --- |
| `ReportDiscoverySnapshot` | `WiDiscReport` | 发现快照上报对象 | `RawInput` 的来源原始对象 | 原始对象保留在 `payload.warp_insight` |
| `DiscoverySnapshot` | `DiscSnap` | 边缘发现快照 | `ResFactSnap` | 不直接等于 atlas 中心资源目录 |
| `DiscoveredResource` | `DiscRes` | 已发现资源 | `ResFact` | 作为资源事实输入，不是中心资源对象 |
| `DiscoveredTarget` | `DiscTarget` | 已发现目标 | `TargetEv` | 只作为目标证据，不直接变成采集配置、服务入口或实例 |
| `CandidateCollectionTarget` | `CollectTarget` | 候选采集目标 | warp-insight planner 产物 | atlas P0 不消费 |
| `CapabilityReport` | `CapReport` | 能力报告 | agent / control capability evidence | 表示 agent 能力和控制面可执行能力，不是 inventory，也不是资源目录 |
| `TelemetryRecord` | `TelemetryRec` | 遥测记录 | telemetry raw input | 不与 discovery snapshot 混用 |
| `ActionPlan` | `ActionPlan` | 动作计划 | 控制面执行对象 | 不进入 atlas topology 主模型 |
| `ActionResult` | `ActionResult` | 动作结果 | 控制面执行结果 | 只可作为审计或证据来源 |

术语边界：

- `DiscoveredResource.resource_id` 是 `warp-insight` 来源侧资源标识。
- `DiscoveredResource.resource_id` 不是 atlas 内部 `host_id`、`pod_id`、`service_id`。
- `DiscoveredTarget.target_id` 是来源侧 target 标识。
- `DiscoveredTarget.target_id` 不是 atlas 采集配置 ID，也不是 endpoint ID。

---

## 5. ID 术语

| 术语 | 中文名 | 示例 | 对外规则 |
| --- | --- | --- | --- |
| `external_id` | 外部 ID | CMDB asset id、IAM user id | 来源系统定义的稳定 ID |
| `external_ref` | 外部引用 | `hostname:office-build-01` | 来源侧可用于引用对象的字符串 |
| `resource_id` | 来源资源 ID | `DiscoveredResource.resource_id` | 只在来源作用域内解释 |
| `target_id` | 来源目标 ID | `DiscoveredTarget.target_id` | 只在来源 target 作用域内解释 |
| `snapshot_id` | 快照 ID | `discovery:42:...` | 表示某轮来源快照 |
| `revision` | 版本推进号 | `42` | 表示来源侧快照推进顺序 |
| `internal_id` | 内部主键 | `host_id`、`service_id` | 只能由 atlas 中心生成，外部输入不得提供或依赖 |

规则：

- atlas 对外文档中不要单独写 `id`，必须写 `external_id`、`external_ref`、`resource_id`、`target_id` 或具体字段名。
- 外部来源 ID 只能在来源作用域内解释，不得当成 atlas 内部主键。
- 外部输入去重应使用来源作用域幂等键，例如 `source.system + producer_id + snapshot_id`。

---

## 6. Snapshot / Batch / Window

| 术语 | 中文名 | 定义 | 示例 |
| --- | --- | --- | --- |
| `Snapshot` | 快照 | 某一时刻完整事实集合 | `DiscoverySnapshot` |
| `Full Sync` | 全量同步 | 某来源的一次完整同步 | CMDB full export |
| `Incremental Sync` | 增量同步 | 按 cursor 推进的变化同步 | IAM updated users |
| `Delta` | 差量事实 | 相对某个基线或 cursor 的新增、更新、删除事实集合 | changed assets since revision 42 |
| `Window` | 时间窗口 | 某时间范围内的聚合摘要 | dependency observation window |
| `Batch Upsert` | 批量幂等导入 | adapter 后的中心 normalized payload | file ingest P0 demo |

禁止混用：

- `Snapshot` 不等于任意 JSON 文件。
- `Batch Upsert` 不等于真实外部采集协议。
- `Delta` 是载荷内容形态；`Incremental Sync` 是同步过程和 cursor 推进机制。
- `Window` 不等于完整快照。

---

## 7. Target / Endpoint / Instance

| 术语 | 中文名 | 对外定义 |
| --- | --- | --- |
| `DiscoveredTarget` | 已发现目标 | 外部来源发现到的候选采集对象线索 |
| `CandidateCollectionTarget` | 候选采集目标 | `warp-insight` planner 为采集生成的目标，atlas P0 不消费 |

边界：

- `DiscoveredTarget` 不能直接变成 atlas 中心服务入口。
- `DiscoveredTarget(kind=service_endpoint)` 可以成为 atlas 内部 endpoint 候选证据，但不能绕过 atlas resolver 直接写成中心 endpoint。
- `DiscoveredTarget` 不能直接变成 atlas 中心服务实例。
- `CandidateCollectionTarget` 是采集规划概念，不是 topology 关系对象。

---

## 8. 推荐表达

推荐：

```text
warp-insight ReportDiscoverySnapshot (WiDiscReport)
  -> atlas RawInput
  -> ResFactSnap
  -> ResFact / RelFact / TargetEv
  -> atlas 内部 adapter / resolver
  -> atlas center source-of-truth / derived view
```

不推荐：

```text
warp-insight resource
  -> atlas resource
```

原因：

- 两边的 `resource` 作用域不同。
- 省略 adapter / resolver 会导致主键、生命周期和置信度语义混乱。

---

## 9. 当前决定

当前阶段固定：

- 跨系统文档必须优先引用本文档。
- `Resource Fact` 作为 `warp-insight DiscoveredResource` 与 atlas 内部处理之间的中间语义。
- `Relation Fact` 作为外部发现关系与 atlas final relation 之间的中间语义。
- `Target Evidence` 作为 `warp-insight DiscoveredTarget` 在 atlas 中的接收语义。
- `ReportDiscoverySnapshot` 是 `warp-insight` discovery 对接的原始对象名，不改名。
- atlas 不复用 `warp-insight` 的本地状态术语来命名中心模型。
