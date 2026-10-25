# wist-atlas 设计文档 Review 结果

Review 日期：2026-04-24

## 总体评价

设计方向正确，领域模型和架构边界的质量较高。五层模型（业务架构、资源目录、运行实例、关系图谱、责任治理）、术语体系、以及"单体优先 -> 按需拆分"的务实策略是整份文档的核心资产。以下按优先级记录需要改进的地方。

---

## 高优先级

### 1. 文档数量过多，存在显著冗余

~30 份文档对应零实现代码。关键概念在多个文档中反复出现：

- 五层模型在 `unified-model-overview.md`、`visual-architecture-guide.md`、`system-architecture.md` 中重复解释
- `dataflow-and-pipeline-architecture.md` 与 `ingest-and-normalization-architecture.md` 在 ingest/normalize 流程上有大量重叠（**已于 2026-04-25 合并**）
- `visual-architecture-guide.md` 的文字论述与其他架构文档重复（**已于 2026-04-25 精简为仅图表**）
- `execution-plan.md` 与 `todo-backlog.md` 结构相似

### 2. Monolith 优先 vs Queue-driven 架构的矛盾

`service-and-deployment-architecture.md` 和 `development-plan.md` 明确说"第一版单体优先"，但 `dataflow-and-pipeline-architecture.md` 的 section 8 花了大量篇幅论述 queue-driven ingest model（protocol registry、partition key、死信队列）。单体模式下 queue-driven 如何运作未被解释。

**建议**：在单体阶段先简化为同步处理链路，queue 作为后续扩展预留。在 dataflow doc 的 queue-driven 章节开头注明"第一版可简化为同步调用，以下为后续扩展设计"。

**状态**：已于 2026-04-25 处理。在 `dataflow-and-pipeline-architecture.md` section 8 开头添加了 note 说明 queue-driven 是逻辑架构不是部署拓扑，单体阶段如何退化处理。在 `service-and-deployment-architecture.md` 形态 A 处添加了对应说明。

### 3. 实现顺序不一致

`unified-model-overview.md:5.4` 的实现顺序有 11 步（host-inventory -> software -> responsibility -> business -> ...），但 `development-plan.md` 和 `execution-plan.md` 的 Phase A 是 `business + host + subject + responsibility` 并行推进。

**建议**：统一到 development-plan 的顺序，更新 unified-model-overview 中的实现顺序建议。

**状态**：已于 2026-04-25 处理。将 `unified-model-overview.md:5.4` 的 11 步线性顺序替换为 Phase A-E 分批方式，与 `development-plan.md` 和 `unified-topology-schema.md:10` 的 Phase 划分对齐。

---

## 中优先级

### 4. Schema 过早精确化 —— 不采纳

`unified-topology-schema.md` 定义了 50+ 张表的精确字段名、索引和外键。

**用户判断**：设计阶段做深入细节设计是正常的，只要后续实现中保持可修改，文档不成为死规范即可。不采纳降级建议。

### 5. 算法推荐过早 —— 不采纳

`dataflow-and-pipeline-architecture.md:7` 推荐了具体算法（MinHash/SimHash、CIDR trie、Jaro-Winkler、Levenshtein 等）。

**用户判断**：设计阶段给出算法方向是正常的，后续实现中可以调整。不采纳移除建议。

### 6. Security 与 Access Control 文档偏薄

`security-and-access-control-architecture.md` 相对其他架构文档明显偏薄。租户隔离被提及但未说明在 storage 层和 query 层的落地方式（row-level security？应用层过滤？）。Explain 查询的独立权限边界只提了概念没有方案。

**建议**：补充租户隔离落地策略和 Explain 权限方案。

### 7. GLOSSARY_SYNC 脚本缺失

`README.md` 和 `glossary.md` 引用了 `scripts/sync_glossary.py`，但该脚本不存在。

**状态**：已于 2026-04-25 补充。脚本位于 `wist-atlas/scripts/sync_glossary.py`，功能：读取 `glossary.md` 的 `GLOSSARY_SYNC_SOURCE` 术语表，同步到所有带 `GLOSSARY_SYNC` 标记的模型文档。幂等设计，重复运行不产生差异。

---

## 低优先级

### 8. 缺少必要的设计章节

以下主题在整个文档集中未被充分讨论：

- **并发模型**：async runtime 选择（tokio？）、actor model？
- **错误处理策略**：统一的错误类型设计、跨 crate 的错误传播（**已于 2026-05-05 补充目标态设计入口：`architecture/error-handling-architecture.md`**）
- **测试策略**：虽然有测试分层提及，但缺乏测试架构文档

### 9. `BusinessHealthFactor` 可能过度设计

五类具体健康因子类型（resource_sufficiency / bug_reduction / vuln_reduction / dependency_stability / threat_reduction）在零数据验证的情况下直接锁定，具体因子类型应随数据接入逐步收敛。

### 10. 缺少从现有系统迁移的讨论

文档假设 greenfield 开发。如有现有 CMDB 或资产系统，迁移路径未讨论。

---

## 已完成的改进（2026-04-25）

- [x] 合并 `ingest-and-normalization-architecture.md` 入 `dataflow-and-pipeline-architecture.md`
- [x] 精简 `visual-architecture-guide.md` 为仅图表 + 简短说明
- [x] 更新所有交叉引用
- [x] 解决 monolith vs queue-driven 矛盾：在 dataflow doc section 8 和 deployment doc 形态 A 处，分别添加说明指出 queue-driven 是逻辑架构，单体阶段各角色共存于同一进程内，"队列"可退化为 DB job table 或进程内 channel

---

## 后续建议

| 优先级 | 行动 |
|--------|------|
| **高** | 解决 monolith vs queue-driven 设计矛盾 |
| **高** | 统一 unified-model-overview 与 development-plan 的实现顺序 |
| **中** | 将 `unified-topology-schema.md` 降级为方向性描述 |
| **中** | 移除或弱化 dataflow doc 中的具体算法推荐 |
| **中** | 补充 Security 文档的租户隔离和 Explain 权限方案 |
| **中** | 实现或移除 `sync_glossary.py` 引用 |
| **低** | 补充并发模型、错误处理、测试架构章节 |
| **低** | 合并 `execution-plan.md` 与 `todo-backlog.md`（或明确其职责差异） |
