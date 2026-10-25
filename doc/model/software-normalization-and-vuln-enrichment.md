# wist-atlas Software Normalization 与 Vulnerability Enrichment 设计

## 1. 文档目的

本文档定义 `wist-atlas` 中“软件归一化”和“漏洞/元数据 enrichment”的独立设计。

这里的目标不是在边缘节点直接查漏洞，而是先把边缘发现到的进程、安装路径和包管理事实，收敛成中心侧可复用的软件实体，再挂载漏洞、生命周期和其他 meta 信息。

本文重点回答：

- `process` / `container` / package facts 如何归一到稳定的软件对象
- 软件知识库应采用什么标识体系
- `CPE`、`purl`、`SWID` 和内部 `product_id / version_id / artifact_id` 的关系是什么
- 漏洞 enrichment 应放在边缘还是中心
- enrichment 结果如何回挂到资源目录和多信号查询体系

相关文档：

- [`glossary.md`](../glossary.md)
- [`../external/warp-insight-edge.md`](../external/warp-insight-edge.md)
- [`../architecture/project-charter.md`](../architecture/project-charter.md)

---

## 2. 核心结论

第一版固定以下结论：

- `wist-atlas` 所依赖的边缘发现侧只负责发现本地软件线索，不负责维护漏洞知识库
- 软件归一化和漏洞 enrichment 应放在中心侧
- 软件实体不能直接以 `CPE` 或 `purl` 作为唯一主键
- 中心必须维护内部稳定 `product_id / version_id / artifact_id`
- 软件必须区分产品、版本和可执行制品，不能把多个版本混成一个对象
- 可执行程序制品必须尽量采集 `sha256`，用于跨主机精确归一
- 脚本也应作为可运行制品建模，记录内容 `sha256`、解释器和来源
- `purl` 和 `cpe_candidates[]` 都应支持，但作用不同
- enrichment 必须先完成 software normalization，再进入 vulnerability lookup

一句话说：

`process / package / file facts -> software normalization -> software entity -> vulnerability enrichment`

而不是：

`process.executable.name -> 直接查 CVE`

---

## 3. 为什么不能直接用进程名查漏洞

直接拿进程路径、可执行名或 `process.executable.name` 去查漏洞，会有明显问题：

- 同一软件在不同机器上的安装路径可能不同
- 同一软件可能有多个 helper / renderer / sidecar 进程
- 一个进程名并不稳定映射到一个发行物
- 进程路径本身通常既不是 `CPE`，也不是 `purl`
- 漏洞库的受影响对象通常按产品、版本、包生态建模，不按运行时进程建模

因此必须固定一个中间层：

- 先把运行时进程和安装线索归一成“软件实体”
- 再把软件实体映射到漏洞情报和生命周期知识

---

## 4. 分层边界

### 4.1 边缘负责什么

边缘负责：

- 发现 `process`
- 发现容器镜像、容器运行时信息
- 发现已安装 package / app bundle / binary path 等本地事实
- 提供 `process.pid`
- 提供 `process.executable.name`
- 提供安装路径、签名、版本线索、哈希等候选事实

边缘不负责：

- 维护完整软件目录
- 维护漏洞知识库
- 做跨主机、跨环境的软件归一化
- 直接把软件线索解释成最终 CVE 结论

### 4.2 中心负责什么

中心负责：

- 软件归一化
- 软件实体建模
- 软件目录存储
- `purl` / `CPE` / `SWID` 外部标识映射
- 漏洞 enrichment
- 生命周期 enrichment
- 许可证、供应商、维护状态等 meta enrichment

---

## 5. 对象模型

### 5.1 `SoftwareEvidence`

这是边缘上报或中心抽取后的“软件识别证据”。

建议结构：

```text
SoftwareEvidence {
  evidence_id
  source_kind
  environment_id
  agent_id
  resource_ref?
  observed_at
  executable_path?
  executable_sha256?
  script_path?
  script_sha256?
  interpreter_path?
  interpreter_sha256?
  install_path?
  binary_name?
  display_name?
  version_text?
  package_name?
  package_manager?
  vendor_hint?
  signer?
  container_image_ref?
  file_hashes?
  raw_facts
}
```

作用：

- 只表达“看到了什么线索”
- 不表达“最终认定它是什么软件”
- 如果 evidence 指向可执行程序，`executable_sha256` 应作为第一优先级采集字段
- 如果 evidence 指向脚本，`script_sha256` 应作为第一优先级采集字段

### 5.2 软件产品、版本、制品分层

第一版必须区分三层：

```text
SoftwareProduct
  -> SoftwareVersion[]
  -> SoftwareArtifact[]
```

含义：

- `SoftwareProduct` 表示“是什么软件”，例如 nginx、OpenSSL、Chrome
- `SoftwareVersion` 表示“哪个版本”，例如 nginx 1.24.0
- `SoftwareArtifact` 表示“哪个具体制品”，例如某个可执行文件、rpm/deb 包、jar、容器镜像层

这样可以避免：

- 同一软件多个版本被混成一个对象
- 只知道进程名时误判漏洞
- 同一版本不同构建或补丁包无法区分
- 可执行文件被替换后仍误认为同一软件

### 5.3 `SoftwareProduct`

表示归一后的软件产品。

```text
SoftwareProduct {
  product_id
  canonical_name
  vendor?
  product_family?
  aliases[]
  homepage?
  state?
  created_at
  updated_at
}
```

### 5.4 `SoftwareVersion`

表示某个软件产品的具体版本。

```text
SoftwareVersion {
  version_id
  product_id
  version
  normalized_version?
  edition?
  release_channel?
  build_metadata?
  state?
  created_at
  updated_at
}
```

建议唯一键：

- `(product_id, normalized_version, edition, release_channel)`

### 5.5 `SoftwareArtifact`

表示某个软件版本对应的具体制品。

```text
SoftwareArtifact {
  artifact_id
  version_id
  artifact_kind
  name?
  path_hint?
  package_type?
  package_name?
  package_manager?
  purl?
  sha256?
  signer?
  signature_status?
  trusted_source?
  interpreter?
  entrypoint?
  permissions?
  size_bytes?
  created_at
  updated_at
}
```

`artifact_kind` 示例：

- `executable`
- `script`
- `shared_library`
- `os_package`
- `language_package`
- `container_image`
- `jar`

字段要求：

- 当 `artifact_kind = executable` 时，`sha256` 应作为必填目标
- 当 `artifact_kind = script` 时，`sha256` 表示脚本内容 hash，也应作为必填目标
- 如果边缘暂时无法计算 hash，可先落 `SoftwareEvidence`，但不要高置信归一到最终 artifact
- `sha256` 用于区分同名、同路径、同版本但内容不同的可执行文件或脚本
- `signature_status` 表示签名校验状态，不应只保存 signer 文本
- `trusted_source` 表示是否来自可信包源、可信镜像源或可信发布渠道

### 5.5.1 `ArtifactVerification`

表示运行中的程序、脚本或包是否被验证为某个可信制品。

建议结构：

```text
ArtifactVerification {
  verification_id
  artifact_id
  host_id?
  pod_id?
  process_id?
  path?
  observed_sha256?
  expected_sha256?
  signature_status?
  signer?
  package_source?
  image_ref?
  attestation_ref?
  verification_level
  result
  confidence
  source
  observed_at
  evidence_ref?
  created_at
}
```

字段说明：

- `observed_sha256` 是运行时实际采集到的文件或脚本 hash
- `expected_sha256` 是中心软件目录或可信来源中的期望 hash
- `signature_status` 表示签名校验结果，例如 valid / invalid / missing / unknown
- `package_source` 表示包管理器或安装来源
- `image_ref` 表示容器镜像来源
- `attestation_ref` 表示远程证明、SBOM、SLSA provenance 或 EDR 证明引用
- `verification_level` 表示验真强度
- `result` 表示是否通过验证

`verification_level` 建议取值：

- `name_only`
- `path_only`
- `hash_match`
- `signature_valid`
- `package_verified`
- `image_verified`
- `attested`

`result` 建议取值：

- `verified`
- `mismatch`
- `unverified`
- `unknown`

验真原则：

- 进程名、脚本名、路径都不能单独证明程序真实
- `sha256` 匹配是第一版最重要的验真依据
- 签名有效、可信包源、可信镜像和远程证明可提高可信度
- 如果运行时 hash 与 catalog hash 不一致，应生成 `mismatch`，不要高置信归一
- 脚本也必须按内容 `sha256` 验真，不能只按脚本路径或名称验真
- 解释器和脚本应分别验真：解释器验解释器 artifact，脚本验脚本 artifact

### 5.5.2 脚本制品建模

脚本应独立建模为 `SoftwareArtifact.artifact_kind = script`。

原因：

- 脚本没有传统安装版本号
- 同名脚本可能在不同主机内容不同
- 脚本内容改动后风险语义会变化
- 脚本依赖解释器，解释器本身也可能有漏洞

脚本制品建议字段：

```text
SoftwareArtifact {
  artifact_id
  version_id?
  artifact_kind = script
  name?
  path_hint?
  sha256
  interpreter?
  interpreter_artifact_id?
  entrypoint?
  permissions?
  signer?
  created_at
  updated_at
}
```

字段说明：

- `sha256` 是脚本内容 hash，不是解释器 hash
- `interpreter` 表示解释器名称或路径，例如 `/bin/bash`、`python3`
- `interpreter_artifact_id` 可指向解释器自身的 `SoftwareArtifact`
- `entrypoint` 表示实际执行入口，例如脚本路径加参数模板
- `permissions` 用于记录是否可执行、owner、mode 等关键权限线索

脚本归一建议：

- 首先按 `sha256` 区分脚本内容
- 再结合路径、名称、来源包、owner、解释器做归一
- 不能只按脚本文件名归一
- 不能只按路径归一，因为路径相同但内容可能变化

脚本风险分析建议：

- 脚本自身可挂载配置风险、恶意脚本、弱权限等 finding
- 解释器漏洞应挂到解释器 artifact 或版本上
- 脚本依赖的语言包或系统命令应通过后续 SBOM / 依赖分析补充

### 5.6 `SoftwareEntity`

这是中心归一后的软件实体。
为兼容已有文档，`SoftwareEntity` 可作为查询视图或兼容别名，实际落库建议拆成 `SoftwareProduct / SoftwareVersion / SoftwareArtifact`。

建议结构：

```text
SoftwareEntity {
  software_id
  canonical_name
  vendor?
  normalized_version?
  edition?
  package_type?
  state?
  aliases[]
  homepage?
  created_at
  updated_at
}
```

关键约束：

- `software_id` 是中心内部稳定主键
- 任何外部标准标识都不直接替代 `software_id`
- 第一版新表设计中，漏洞匹配应优先落到 `SoftwareVersion`，可执行文件精确匹配落到 `SoftwareArtifact`

### 5.7 `SoftwareExternalId`

建议单独建模外部标识映射。

```text
SoftwareExternalId {
  target_type
  target_id
  id_kind
  id_value
  confidence
  source
  observed_at
}
```

其中：

- `id_kind` 可取：
  - `purl`
  - `cpe`
  - `swid`
  - `vendor_product_id`
- `target_type` 可取 `product`、`version`、`artifact`
- `confidence` 表达匹配可信度

### 5.8 `SoftwareVulnerabilityFinding`

这是 enrichment 后的漏洞结果。

```text
SoftwareVulnerabilityFinding {
  finding_id
  version_id
  artifact_id?
  advisory_source
  vulnerability_id
  aliases[]
  severity
  affected_range?
  fixed_range?
  exploited_known?
  patch_available?
  published_at?
  updated_at?
  evidence
}
```

关键约束：

- 这层是 enrichment 结果，不是原始情报镜像
- `evidence` 应说明本次是基于 `purl`、`CPE` 还是 vendor feed 命中的
- 漏洞通常命中 `version_id`
- 如果漏洞或风险与具体文件 hash 相关，可进一步命中 `artifact_id`

### 5.9 `SoftwareBug`

表示软件缺陷。

`SoftwareBug` 不等同于漏洞：

- BUG 是软件缺陷事实
- 漏洞是具有安全影响的缺陷或弱点
- 一个 BUG 可能没有安全影响
- 一个漏洞 finding 可能来自一个或多个 BUG、CVE、advisory 或 vendor issue

建议结构：

```text
SoftwareBug {
  bug_id
  product_id
  version_id?
  artifact_id?
  bug_key?
  title
  description?
  bug_type
  severity?
  priority?
  status
  affected_range?
  fixed_range?
  fixed_version_id?
  source
  external_ref?
  discovered_at?
  fixed_at?
  created_at
  updated_at
}
```

字段说明：

- `bug_id` 是中心内部 BUG 主键
- `bug_key` 是来源系统里的缺陷编号，例如 Jira issue、GitHub issue、vendor bug id
- `product_id` 表示该 BUG 属于哪个软件产品
- `version_id` 表示已确认影响的具体版本
- `artifact_id` 表示只影响某个具体文件、脚本、包或镜像制品
- `affected_range` / `fixed_range` 表示影响和修复版本范围
- `fixed_version_id` 表示中心已归一出的修复版本

`bug_type` 示例：

- `functional`
- `crash`
- `performance`
- `compatibility`
- `data_corruption`
- `security`
- `config`
- `script`

`status` 示例：

- `open`
- `confirmed`
- `fixed`
- `wont_fix`
- `duplicate`
- `unknown`

### 5.10 `SoftwareBugFinding`

表示某个资源、进程、版本或制品命中了某个 BUG。

`SoftwareBug` 是缺陷目录对象，`SoftwareBugFinding` 是命中事实。

建议结构：

```text
SoftwareBugFinding {
  finding_id
  bug_id
  product_id
  version_id?
  artifact_id?
  host_id?
  pod_id?
  process_id?
  evidence_id?
  confidence
  status
  first_seen_at
  last_seen_at
  created_at
  updated_at
}
```

说明：

- 一个 BUG 可影响多个版本、制品和主机
- 一个 finding 表示“这里确实命中了这个 BUG”
- 对脚本类 BUG，应优先通过 `artifact_id + sha256` 精确命中
- 对版本类 BUG，应通过 `version_id` 和版本范围命中
- 对配置或运行条件触发的 BUG，需要 evidence 支撑

### 5.11 `SoftwareBug` 与漏洞 finding 的关系

建议用独立关系表达 BUG 与漏洞的关联：

```text
SoftwareBugVulnLink {
  bug_id
  finding_id
  relation_type
  confidence
  source
  created_at
}
```

`relation_type` 示例：

- `causes`
- `fixed_by_same_patch`
- `references`
- `duplicate`
- `unknown`

建模原则：

- 普通 BUG 不进入漏洞 finding
- 有安全影响的 BUG 可以关联到 `SoftwareVulnerabilityFinding`
- CVE 不应直接替代 BUG 编号
- vendor issue、GitHub issue、Jira issue、CVE/advisory 应作为不同来源保留

### 5.12 错误日志如何形成 BUG

错误日志可以用于发现 BUG，但不应由单条错误日志直接创建 `SoftwareBug`。

推荐链路：

```text
error log / crash dump / exception / core signal
  -> BugEvidence
  -> BugObs
  -> SoftwareBugFinding
  -> SoftwareBug?
```

含义：

- `BugEvidence` 表示一条具体错误证据，例如异常栈、错误码、crash dump、panic 日志
- `BugObs` 表示错误观测摘要，例如同一错误签名在某个时间窗口内出现了多少次
- `SoftwareBugFinding` 表示某个版本、制品、主机或进程命中了某个已知或候选 BUG
- `SoftwareBug` 是归一后的缺陷目录对象，只有证据足够稳定时才创建或关联

第一版可以不单独落 `BugEvidence / BugObs` 表，但概念上应保留这两层，避免把日志明细直接塞进 `SoftwareBug`。

错误日志生成 BUG 的流程：

```text
parse error log
  -> normalize stack / error code / message
  -> compute error_signature
  -> attach product/version/artifact
  -> aggregate by time window
  -> match known SoftwareBug
  -> create or refresh SoftwareBugFinding
  -> create provisional SoftwareBug if repeated and stable
```

`error_signature` 建议由以下字段组合生成：

- 归一后的异常类型
- 归一后的错误码
- 栈顶关键帧
- 崩溃函数或模块
- artifact `sha256`
- 版本号
- 归一后的错误消息模板

可作为 BUG 信号的来源：

- 应用错误日志
- panic / exception stack trace
- crash dump / core dump 摘要
- 进程异常退出码
- kernel / runtime 错误事件
- 用户工单或告警归因结果
- vendor issue / GitHub issue / Jira issue

不能直接创建 BUG 的信号：

- 单条孤立错误日志
- 只有超时但没有明确错误签名
- 只有 HTTP 5xx 但无法归因到软件版本或制品
- 由外部依赖不可用导致的错误
- 配置错误、容量不足或环境问题，但没有软件缺陷证据

创建 `SoftwareBug` 的建议条件：

- 同一 `error_signature` 在多个窗口重复出现
- 能关联到明确 `product_id`，最好能关联到 `version_id` 或 `artifact_id`
- 排除了环境、配置、依赖不可用等非软件缺陷原因
- 有足够证据生成 `title / bug_type / source`

`SoftwareBugFinding` 的生成条件可以更宽：

- 已知 BUG 规则命中
- 或错误签名命中候选 BUG
- 或某个 artifact `sha256` 命中已知问题脚本/二进制

状态建议：

- 证据不足时，`SoftwareBug.status = unknown` 或不创建 `SoftwareBug`
- 候选命中时，`SoftwareBugFinding.status = suspected`
- 规则或人工确认后，`SoftwareBugFinding.status = confirmed`
- 修复版本或补丁验证后，`SoftwareBug.status = fixed`

---

**图：软件与漏洞 ER 关系**

```mermaid
erDiagram
  SoftwareProduct ||--o{ SoftwareVersion : has-version
  SoftwareVersion ||--o{ SoftwareArtifact : has-artifact
  SoftwareArtifact ||--o{ ArtifactVerification : verified-by
  SoftwareVersion ||--o{ SoftwareVulnerabilityFinding : affected-by
  SoftwareArtifact ||--o{ SoftwareVulnerabilityFinding : affected-by
  SoftwareVersion ||--o{ SoftwareBugFinding : has-bug
  SoftwareArtifact ||--o{ SoftwareBugFinding : has-bug
```

> 软件按 `Product → Version → Artifact` 三层归一。漏洞/Bug 通常挂在 `SoftwareVersion` 上，必要时精确到 `SoftwareArtifact`。`ArtifactVerification` 通过 hash/签名/包源验证制品可信性。

---

## 6. 标识体系

### 6.1 内部主键

必须明确：

- `product_id` 是软件产品主键
- `version_id` 是软件版本主键
- `artifact_id` 是软件制品主键
- 不能直接把 `CPE` 作为内部主键
- 不能直接把 `purl` 作为内部主键

原因：

- 一个软件产品可能对应多个版本
- 一个软件版本可能对应多个制品
- 一个软件版本可能对应多个 `CPE`
- 一个软件制品可能有多个 `purl`
- 同一产品不同来源可能存在别名和版本表达差异

### 6.2 `purl`

`purl` 更适合现代包生态。

适合场景：

- OS package
- Java / npm / PyPI / Go module
- 容器镜像中的 package
- 语言生态依赖

优势：

- 语义更现代
- 对 package ecosystem 更自然
- 适合和 SBOM 及包管理系统衔接

限制：

- 不覆盖所有传统桌面 app / 手工安装二进制
- 与 CVE/NVD 体系不是一一直接对齐

### 6.3 `CPE`

`CPE` 更适合漏洞情报映射。

适合场景：

- 对接 NVD
- 对接以产品/版本为核心的漏洞知识库
- 对接传统安全产品接口

优势：

- 在 CVE / NVD 场景中兼容性强

限制：

- 现代软件生态映射常常不自然
- 命名歧义较多
- 同一软件常常需要维护多个 `cpe_candidates[]`

### 6.4 `SWID`

`SWID` 可作为企业资产或安装清单补充标识，但不建议作为第一版主路径。

### 6.5 结论

第一版建议采用：

- 内部：`product_id / version_id / artifact_id`
- 现代生态映射：`purl`
- 漏洞情报映射：`cpe_candidates[]`
- 预留：`swid`

一句话说：

- 内部靠 `product_id / version_id / artifact_id`
- 包生态优先 `purl`
- 漏洞库兼容 `CPE`

---

## 7. 归一化流程

建议固定如下处理链：

```text
SoftwareEvidence ingest
-> evidence canonicalization
-> software candidate match
-> product / version / artifact merge or create
-> external id mapping
-> vulnerability enrichment
-> resource / process / package back-reference
```

### 7.1 evidence canonicalization

处理内容：

- 路径标准化
- 版本文本清洗
- 供应商别名统一
- 可执行名与 bundle 名归一
- package manager 名称统一

### 7.2 software candidate match

建议按以下优先级：

1. executable / script `sha256`
2. package manager + package name + version
3. signed bundle / product metadata
4. executable path + version metadata
5. hash + known catalog
6. heuristic alias mapping

### 7.3 merge / create

如果已有高置信匹配：

- 挂到现有 `artifact_id`、`version_id` 或 `product_id`

如果没有可靠匹配：

- 生成新的 `product_id / version_id / artifact_id`
- 标记 `normalization_status = provisional`

### 7.4 external id mapping

对同一产品、版本或制品：

- 生成或关联 `purl`
- 生成或关联一个或多个 `cpe_candidates`
- 记录匹配置信度和来源

### 7.5 vulnerability enrichment

只有在有足够可靠的外部标识后才进入漏洞 enrichment。

---

## 8. 漏洞 enrichment 设计

### 8.1 数据源分层

建议支持三类来源：

- `vendor advisory`
- `ecosystem advisory`
- `CVE/NVD`

优先级建议：

1. vendor advisory
2. ecosystem advisory
3. NVD / generic CVE feed

原因：

- vendor feed 通常更贴近真实产品版本
- ecosystem advisory 更适合语言包和依赖
- NVD 兼容性广，但匹配误差更大

### 8.2 enrichment 结果不直接等于最终风险结论

必须区分：

- 漏洞命中事实
- 运营风险结论

中间还需要结合：

- 软件是否真的安装
- 版本是否准确
- 是否在运行
- 是否暴露
- 是否有 exploit
- 是否有 patch

因此 `SoftwareVulnerabilityFinding` 只是 enrichment 事实，不直接等同“高风险告警”。

---

## 9. 与资源目录的关系

建议新增一个软件关联层，而不是直接把软件属性全部塞进 `DiscoveredResource`。

### 9.1 关联关系

```text
ProcessResource -> SoftwareEvidence -> SoftwareArtifact -> SoftwareVersion -> VulnerabilityFindings
ContainerImage -> SoftwareArtifact -> SoftwareVersion -> VulnerabilityFindings
InstalledPackage -> SoftwareArtifact -> SoftwareVersion -> VulnerabilityFindings
```

### 9.2 回挂方式

中心查询时可为 `process` / `host` / `container` 附加：

- `product_ref`
- `version_ref`
- `artifact_ref`
- `software.name`
- `software.vendor`
- `software.version`
- `software.sha256`
- `software.state`
- `software.vulnerability_summary`

但边缘本地 cache 不应直接变成漏洞数据库。

---

## 10. 边缘上报建议

第一版边缘只需补充足够的 `SoftwareEvidence` 候选事实，不必一开始就做完整 package inventory。

建议最小上报字段：

- `process.executable.name`
- `process.pid`
- `resource_id`
- 安装路径或 bundle 路径
- 可执行文件 `sha256`（若能低成本获取）
- 脚本路径、脚本内容 `sha256`、解释器路径（若进程由脚本启动）
- 版本候选文本（若能低成本获取）
- signer / package manager / image ref（若能低成本获取）

边缘原则：

- 低开销
- 不阻塞采集主路径
- 不因 enrichment 失败影响数据面

---

## 11. 存储建议

第一版中心存储建议拆成：

- `software_product`
- `software_version`
- `software_artifact`
- `software_aliases`
- `software_external_ids`
- `software_evidence`
- `software_vulnerability_findings`
- `software_lifecycle_facts`

如果后续接 SBOM，可再增加：

- `software_components`
- `software_dependency_edges`

---

## 12. 第一版落地范围

第一版建议只做：

1. `process -> SoftwareEvidence`
2. 基于路径 / bundle / package name 的初步 software normalization
3. 内部 `product_id / version_id / artifact_id`
4. `purl` 与 `cpe_candidates[]` 的最小映射结构
5. 漏洞 enrichment 存储骨架
6. 可执行文件和脚本的 `sha256` 采集与归一

第一版不建议一开始就做：

- 全量 SBOM ingestion
- 软件依赖图
- 实时 exploit intelligence
- 复杂许可证合规分析

---

## 13. 验收标准

第一版至少应满足：

- 同一软件的多个 helper / renderer 进程能归到同一个 `product_id`
- 同一产品的多个版本能区分到不同 `version_id`
- 同一路径下内容不同的可执行文件或脚本能通过 `sha256` 区分到不同 `artifact_id`
- 带空格路径、bundle app、package manager 安装项能稳定识别
- `product_id / version_id / artifact_id` 与 `purl` / `cpe_candidates[]` 可同时存在
- 漏洞 enrichment 结果能说明命中来源和置信度
- enrichment 失败不影响边缘 discovery 和 telemetry 主路径

---

## 14. 当前建议

当前建议固定为：

- 软件归一化采用“内部 `product_id / version_id / artifact_id` + `purl` + `cpe_candidates[]`”
- 不采用单一 `CPE` 作为唯一标准
- 边缘只提供 evidence
- 中心完成 software normalization 与 vulnerability enrichment

一句话总结：

`wist-atlas` 应把软件知识库设计成中心侧产品、版本、制品目录，而不是把进程名直接当漏洞库查询键。
