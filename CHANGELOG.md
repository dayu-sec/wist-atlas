# 更新日志

本文件记录 `wist-atlas` 的所有重要变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

### 改进（拓扑可视化视觉改版）

细节与参考产品见 [`doc/frontend/visual-design.md`](doc/frontend/visual-design.md)。

- **构图从「圆环」改成「横向剖面」**：网络接口（左）→ 主机（中）→ 运行时（右）。
  上一版把 8 个网段绕着主机摆成椭圆，每条边等长、方向随机，读不出结构；
  现在两侧栏有标题、计数和分隔线，宽屏也用上了。
- **节点从几何体改成信息卡**：接口名从关联边 `host_network_assoc.properties.ifaceName`
  读出来做标题，CIDR 做副标题；主机卡带系统/架构/盘点时间；运行时卡带
  「1,120 进程 / 705 程序 / 高频程序 Top 3」。颜色只留在 28–44px 的图标块里。
- **页面壳收敛**：56px 深色顶栏 + 44px 工具条改成 52px 顶栏 + 画布左上浮动工具条；
  左栏变成主机档案（身份 / 四个指标 / 高频进程条形图 / 图例 / 对象查询）。
- **搜索索引补上边属性**：`bridge`、`en0`、CIDR 都能搜到网段；零命中时不再把整张图压暗。
- **运行时展开**：卡片正文看详情、底部按钮展开；12 个分组按两列网格铺开（约 76%），
  不再被 dagre 排成一列后缩到 39%。
- **修复**：
  - 选中节点 / 输入搜索会导致整图重新布局并重置视口（`displayGraph` 身份不稳定）；
  - cytoscape 把容器 inline `position` 改成 `relative`，`absolute inset-0` 高度塌成 0，
    fit 只横向居中，图被顶到画布上沿；
  - 图层开关只改 state、画布没消费；
  - 浮动工具条压住栏标题。
- 新增 `visualization/src/utils/focus-layout.ts`、`presentation.ts`、`search.ts`，
  以及 `components/NodeCard.tsx` / `AppHeader.tsx` / `icons.tsx`。

### 新增（主机架构与退役）

- **`host_inventory` 加 `arch`**：CPU 架构（`aarch64` / `x86_64`）。包与二进制匹配、
  漏洞影响面判断都要它 —— 只知道「一台 macOS 26.6.2 的机器」分不出该发哪种产物。
  取值与事实摘要的 `arch` 同源（`std::env::consts::ARCH`）。
  HTTP 的 host 视图也带上了（`arch`）。
- **主机退役（stale）**：与进程**判据不同**。进程按「一份快照没出现」算；主机按
  **连续 N 份完整快照都没出现**（`HOST_STALE_AFTER_MISSES`，默认 3）——
  主机消失可能只是一轮探针失败，误标一台活主机的代价远大于晚几轮才标。
  - 看到即归零，退役后冻结首次时间、不再累加；
  - 只动同作用域的行；存量行没有作用域就不参与清扫（宁可不标也不误标）；
  - 查询与导入汇总默认不返回退役主机。
  - **限制**：需要多主机快照才会触发。当前适配器一台机一个信封，所以这条清扫
    要等网关侧聚合 / CMDB 同步这类输入才会真正生效。
- 迁移 `0003_host_arch_presence`：`host_inventory` 加 `arch` / `stale_at` /
  `ingest_scope` / `miss_count`（`DEFAULT 0`）与两个部分索引，幂等。

### 新增（自动消费网关 sink 文件）

- **新增 crate `atlas-wist-ingest` / 可执行文件 `atlas-wist-ingest`**：增量读网关
  `agent-discovery` sink 落下的文件，转成 `atlas.in.edge.v1` 直接入库。从此不需要有人手跑
  适配器、手拷文件、手敲 `import-jsonl`。
  - **不丢帧**：网关的 `http_sink` 只有 `max_retries = 3`，之后帧就没了；sink 文件是追加写的，
    atlas 停机期间帧还在长，恢复后自动补。
  - **不需要新端口与新鉴权**：信任边界就是文件权限；写 atlas 的只有 atlas 自己。
  - 偏移检查点原子写；只消费完整行（半写中的帧不会进解析器）；轮转/截断自动从 0 重读。
  - 失败分两类：格式坏行跳过并推进（永远修不好），写库失败不推进（下一轮重试）。
  - 配套：`atlas-wist-adapter` 新增 `envelope_from_sink_line`（sink 行里的 `body` 是**字符串**，
    和 `DSNAP:` 帧的裸正文不是同一形状）。

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
