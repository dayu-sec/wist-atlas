# 更新日志

本文件记录 `wist-atlas` 的所有重要变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

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
