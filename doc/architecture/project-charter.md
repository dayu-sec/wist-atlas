# wist-atlas 项目 Charter

## 1. 项目目标

`wist-atlas` 的目标是建立一套统一的中心侧对象模型与关系图谱，覆盖：

- 业务与系统拓扑
- 服务与实例拓扑
- 主机、Pod、网络拓扑
- 软件归一化与漏洞关联
- 责任归属与外部系统同步

## 2. 它是什么

它是：

- 统一资产与运行拓扑中心
- 统一目录对象与关系对象模型
- 面向查询、治理与分析的中心侧底座

## 3. 它不是什么

它不是：

- 边缘 agent 仓库
- 控制平面执行仓库
- 完整数字孪生仿真系统
- 原始 telemetry 数据面仓库

## 4. 第一版核心模块

- Model Catalog
- Topology Ingest
- External Sync
- Query API
- Governance Relations

## 5. 第一版核心对象域

- `business / system / service`
- `host / pod / network`
- `software / vulnerability`
- `responsibility`

## 6. 第一版存储建议

- PostgreSQL 作为主存储
- 对象存储保存原始导入载荷与快照
- 缓存按需引入，不作为 source of truth

## 7. 第一版原则

- 先统一模型，再扩展实现
- 先做 source of truth，再做派生视图
- 先做清晰边界，再做大而全整合
