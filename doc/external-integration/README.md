# 外部对接规范

本目录集中维护 `wist-atlas` 与外部系统、采集器、同步源之间的数据对接规范。

文档入口：

- [`input-taxonomy-and-style.md`](./input-taxonomy-and-style.md)：基于 `doc/model` 的输入分类、JSON 样式和 adapter 输出口径
- [`external-glossary.md`](./external-glossary.md)：外部对接术语、跨系统术语对齐和禁止混用规则；不包含 atlas 内部 pipeline 对象
- [`external-input-spec.md`](./external-input-spec.md)：外部 raw input envelope 与来源协议规范
- [`warp-insight-to-atlas-dataflow.md`](./warp-insight-to-atlas-dataflow.md)：`warp-insight` 经 `warp-parse` 进入 `wist-atlas` 的当前真实数据流、导入链路和落库状态
- [`warp-insight-adapter-spec.md`](./warp-insight-adapter-spec.md)：`warp-insight` discovery 快照到 atlas candidate / evidence 的映射规范
- [`file-ingest-spec.md`](./file-ingest-spec.md)：adapter 之后的 normalized batch import payload 规范
- [`../internal/processing-glossary.md`](../internal/processing-glossary.md)：atlas 内部 adapter / resolver / materializer 术语、短名和字段命名规则

示例数据：

- [`../../fixtures/external-input/target`](../../fixtures/external-input/target)
- [`../../fixtures/file-ingest`](../../fixtures/file-ingest)

边界说明：

- 本目录定义外部输入和对接样式。
- 外部系统只需要遵守 external raw input / source facts / envelope 规范。
- atlas 内部 `*Cand`、`*Ev`、lower_snake 输出集合名不属于外部接口契约。
- `doc/model` 定义中心模型对象和关系。
- `doc/architecture` 定义系统架构、pipeline、存储、同步和查询边界。
