# 更新日志

本文件记录 `wist-atlas` 的所有重要变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

### 新增（快照替换语义）

- **`mode = snapshot` 现在会标记「这次没出现的进程」为 stale**，不再让退出的进程永久停在
  资产目录里。之前 `process_id` 是稳定 UUID + `ON CONFLICT DO UPDATE`，所以退出的进程
  只会被覆盖、永远不会消失 —— pid 一换，目录很快就全是死进程。
  - 按作用域 `(schema, system, producer, tenant, env)` 划界，同一台主机上其它生产者
    （如容器采集器）报的进程不会被误标。
  - **不删行**：只写 `stale_at`（首次消失时间）；查询与导入汇总默认不返回已 stale 的行。
  - 进程重新出现时 `stale_at` 自动清掉；`observed_at` 保持最后一次真实观测时间。
  - 三条防误标约束：空列表不清扫、有对象解析不到宿主就整体跳过、只动本次快照覆盖到的主机。
  - `incremental` / `window` / `correction` 不做清扫（部分观测不能当全集比对）。
- 迁移 `0002_presence_stale`：`process_runtime_state` 加 `stale_at` / `ingest_scope`
  两列与两个部分索引，幂等（`ADD COLUMN IF NOT EXISTS`）。

### 变更（适配器移植到 Rust）

- **发现快照适配器改用 Rust 实现**：新增 crate `atlas-wist-adapter` / 可执行文件
  `atlas-wist-adapter`，替换 0.1.0 的 `scripts/wist-snapshot-to-atlas.py`。
  - 命令行开关（`--state-dir` / `--report` / `--agent-id` / `--tenant` / `--env` / `--out` / `--jsonl`）
    与 `atlas.in.edge.v1` 输出**逐字节一致**，切换不需要改调用方。
  - 换成 Rust 的理由：映射规则能被 `cargo test` 守住、能进 CI，而一个脚本改错一个属性名
    只会静默产出空资产。
  - 输入解析**故意**不用 `wist-contracts` 的类型：那些类型带 `deny_unknown_fields`，
    对新版 agentd 多出来的字段会把**整帧**打回 —— 对消费端那是反的。
  - 裁剪过的 macOS fixture（1 host / 2 网卡 / 3 进程）作为 golden，改映射规则必须同步重新生成。

## [0.1.0-alpha] - 2026-10-25

首个以 `wist-atlas` 之名发布的版本（前身 `dayu-topology`）。

### 变更（仓库改名）

- **仓库与制品改名**：`dayu-topology` → `wist-atlas`；crate `topology-*` → `atlas-*`；
  可执行文件 `wist-atlas`；数据库 `wist_atlas`、用户 `atlas`。
- **输入 schema**：`atlas.in.*`。旧 `dayu.in.*` 仍被接受，改名期间的存量投递不会被打回。
- **错误码**：`*.dayu.*` → `*.atlas.*`。
- **环境变量**：`WIST_ATLAS_*`；旧 `DAYU_*` 作为兜底保留。

### 新增（边缘资产接入）

- **接受 `processes[]` 数组**：一个快照里的全部进程可以一次投递，不必每进程一个信封。
  数组与旧的单进程对象同时存在时**以数组为准**，不会重复计数。
- **`pid` 接受数字或字符串**：目标态输入用数字（`"pid": 210`），存量样例用字符串，两者都收。
- **`host_ref` 作为主机名回退**：在 `host_name` / `hostname` 之后、`process_key` 推导之前。
- **Wist 边缘快照适配器**（`scripts/wist-snapshot-to-atlas.py`）：把 `wist-agentd` 的发现快照
  （`--state-dir` 的 state 目录，或 `--report` 的 `DSNAP:` 帧体）转成 `atlas.in.edge.v1` 信封。

### 说明

- 本仓尚无 CI 与打包流水线：`version.txt` 目前只是版本记录，不产出发布附件。
