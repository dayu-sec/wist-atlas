# wist-atlas 文档索引

## 目录说明

- `architecture/`
  项目定位、边界、模块拆分、存储与服务边界
- `model/`
  核心对象模型、关系模型、拓扑模型与治理模型
- `external-integration/`
  外部输入、connector 对接、文件导入 payload 与样式规范
- `internal/`
  内部 pipeline、处理术语、代码命名与实现约束
- `frontend/`
  可视化前端架构与前后端对接契约
- `roadmap/`
  迭代计划、阶段目标、落地顺序

## 设计 Review

设计文档的整体 review 结果和待改进项记录在 [review-findings.md](./review-findings.md)。

## 建议阅读顺序

1. [glossary.md](./glossary.md)
2. [architecture/project-charter.md](./architecture/project-charter.md)
3. [architecture/system-architecture.md](./architecture/system-architecture.md)
4. [architecture/visual-architecture-guide.md](./architecture/visual-architecture-guide.md)
5. [architecture/storage-architecture.md](./architecture/storage-architecture.md)
6. [architecture/service-and-deployment-architecture.md](./architecture/service-and-deployment-architecture.md)
7. [architecture/dataflow-and-pipeline-architecture.md](./architecture/dataflow-and-pipeline-architecture.md)（包含 `wp-reactor` 规则计算引擎边界）
8. [external-integration/README.md](./external-integration/README.md)
9. [external-integration/input-taxonomy-and-style.md](./external-integration/input-taxonomy-and-style.md)
10. [external-integration/external-input-spec.md](./external-integration/external-input-spec.md)
11. [external-integration/file-ingest-spec.md](./external-integration/file-ingest-spec.md)
12. [internal/README.md](./internal/README.md)
13. [internal/processing-glossary.md](./internal/processing-glossary.md)
14. [architecture/scenario-and-scope-model.md](./architecture/scenario-and-scope-model.md)
15. [architecture/network-modeling-analysis.md](./architecture/network-modeling-analysis.md)
16. [architecture/external-sync-architecture.md](./architecture/external-sync-architecture.md)
17. [architecture/query-and-read-model-architecture.md](./architecture/query-and-read-model-architecture.md)
18. [architecture/identity-resolution-architecture.md](./architecture/identity-resolution-architecture.md)
19. [architecture/error-handling-architecture.md](./architecture/error-handling-architecture.md)
20. [architecture/security-and-access-control-architecture.md](./architecture/security-and-access-control-architecture.md)
21. [architecture/observability-and-audit-architecture.md](./architecture/observability-and-audit-architecture.md)
22. [architecture/unified-model-overview.md](./architecture/unified-model-overview.md)
23. [model/README.md](./model/README.md)
24. [model/unified-topology-schema.md](./model/unified-topology-schema.md)
25. [roadmap/bootstrap-plan.md](./roadmap/bootstrap-plan.md)
26. [roadmap/development-plan.md](./roadmap/development-plan.md)
27. [roadmap/execution-plan.md](./roadmap/execution-plan.md)
28. [roadmap/todo-backlog.md](./roadmap/todo-backlog.md)
29. [frontend/architecture.md](./frontend/architecture.md)
30. [frontend/api-contract.md](./frontend/api-contract.md)

## Glossary 同步

当 `doc/glossary.md` 中的标准术语表发生变化后，可运行：

```bash
python3 scripts/sync_glossary.py
```

用途：

- 把 glossary 中的标准术语中英说明同步到带有 `GLOSSARY_SYNC` 标记的文档
- 避免多个模型文档各自维护一份术语解释
- 让 glossary 成为单一术语源
