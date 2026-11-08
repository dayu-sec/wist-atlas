-- 软件指纹与实例（客户域），外加内容寻址的 artifact 字典。
--
-- 设计：doc/design/center/models/software-fingerprint-verification.md
--
-- 一句话：**身份由字节定，不由名字定**。
--   主机上「重要二进制」的 sha256 就是这个软件实例的唯一权威身份 ID；
--   产品名 / 版本 / 安装路径 / unit 名都只是属性或位置锚点，可以缺、可以错，
--   但不能反过来替代字节做身份。
--
-- 为什么要有 `software_instance` 而没有 `software_installation`：
--   1. 部署单元（systemd unit / 容器 / socket）没有标准，必须有位置锚点才能成为图谱节点；
--   2. 「包 × 主机」那层的每个字段在 instance 上都有归宿（claimed_*），多一张表只多一条 join；
--   3. 「装了但没跑」已被范围文档划出图谱（必须持有活进程），installation 唯一的独占场景不在范围内；
--   4. 6 个官方镜像里 5 个不打包自己（kafka/ES/redis/rabbitmq/openjdk），
--      tarball / 容器 / 自编译根本没有包记录 —— installation 在最重要的目标上会是空表。
--
-- 0006 只落**客户域 + artifact 字典**。知识库（product / version / advisory / recognize /
-- bundled / profile / kb_state）是独立的一版，导入语义也不同（整表替换 vs 增量 upsert），
-- 见 doc/design/center/models/software-kb-import.md。

-- 一份字节 = 一次构建。artifact_id 内容寻址 `sha256:<hex>`，与现场上报同值 ——
-- 不另设内部 UUID，否则中心要多一层翻译，两个 ID 迟早会漂，
-- 而「同一份字节两个身份」正是这套设计最想防的错。
CREATE TABLE IF NOT EXISTS software_artifact (
    artifact_id TEXT PRIMARY KEY,
    -- 引用知识库的版本（software_version）。KB 导入落地前允许为空：
    -- 现场上报的字节可能还没被可信源收录，此时 status=observed，ID 依然有效。
    version_id TEXT,
    artifact_kind TEXT NOT NULL DEFAULT 'executable',
    sha256 TEXT NOT NULL,
    size_bytes BIGINT,
    -- ELF .note.gnu.build-id：重新链接仍稳定，可作 sha256 之外的辅助判据。
    build_id TEXT,
    package_manager TEXT,
    package_name TEXT,
    purl TEXT,
    -- 脚本的字节由解释器执行；指向另一个 software_artifact。
    interpreter_artifact_id TEXT,
    -- 构建坐标 {distro_id, distro_version, component, arch, libc}。
    -- 取自**制品元数据**，不是宿主 /etc/os-release：在 Debian 12 上跑官方 tarball 的
    -- Kafka，宿主 distro 指向的是一条不存在的构建。
    coordinate JSONB,
    first_seen_at TIMESTAMPTZ,
    last_seen_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_software_artifact_sha256
    ON software_artifact(sha256);
CREATE INDEX IF NOT EXISTS idx_software_artifact_version_id
    ON software_artifact(version_id)
    WHERE version_id IS NOT NULL;

-- 一台主机上的一个部署位。
CREATE TABLE IF NOT EXISTS software_instance (
    instance_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    host_id UUID NOT NULL REFERENCES host_inventory(host_id),
    anchor_kind TEXT NOT NULL,
    anchor_value TEXT NOT NULL,
    -- 权威身份；为空 = 算不出 hash（无权限 / memfd / 超上限 / 容器内不可读）。
    current_artifact_id TEXT,
    identity_missing_reason TEXT,
    -- 声称值（包反查 / 版本探测 / 镜像 tag）。
    -- 包名与产品名不合并：`postgresql-16` 是包、`postgresql` 是产品，
    -- 而补丁与 advisory 按包发。
    claimed_package_name TEXT,
    claimed_product_id TEXT,
    -- 原样版本串；来自包元数据时它就是 version_installed。
    claimed_version TEXT,
    claimed_source TEXT,
    -- 入库凭据：JSON 数组，解释「这条为什么在图上」。
    role TEXT NOT NULL DEFAULT '[]',
    reason TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    -- 退役时刻；NULL = 现役。与进程的 stale 同一口径：行保留以备回溯，不进现役视图。
    stale_at TIMESTAMPTZ,
    CONSTRAINT software_instance_anchor_unique
        UNIQUE (host_id, anchor_kind, anchor_value),
    -- 允许「没有 ID」，但不允许「假装有 ID」：算不出 hash 必须说明原因。
    CONSTRAINT software_instance_identity_or_reason
        CHECK (current_artifact_id IS NOT NULL OR identity_missing_reason IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_software_instance_host_id
    ON software_instance(host_id);
CREATE INDEX IF NOT EXISTS idx_software_instance_artifact_id
    ON software_instance(current_artifact_id)
    WHERE current_artifact_id IS NOT NULL;

-- host × artifact：这台机上出现过哪些字节。用于去重与跨机 / 跨租户同源。
CREATE TABLE IF NOT EXISTS software_fingerprint (
    tenant_id UUID NOT NULL,
    host_id UUID NOT NULL,
    artifact_id TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    build_id TEXT,
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id, host_id, artifact_id)
);

CREATE INDEX IF NOT EXISTS idx_software_fingerprint_artifact_id
    ON software_fingerprint(artifact_id);

-- 观测时间线：回答「哪天升的、升到哪份字节」。
-- 变化检测要归因（hash 变 × 包版本 × 变更日志），没有这条时间线就没有基线。
CREATE TABLE IF NOT EXISTS software_instance_observation (
    instance_id UUID NOT NULL REFERENCES software_instance(instance_id),
    artifact_id TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    version_running TEXT,
    version_installed TEXT,
    PRIMARY KEY (instance_id, artifact_id, observed_at)
);

-- 本机报告里的路径明细（默认不上送）。
-- 只有 identity_missing 与「边缘判出与包记录不符」时随帧上报：
-- 路径一旦被当作去重键，容器内每轮变化的路径会把去重打散。
CREATE TABLE IF NOT EXISTS software_artifact_path (
    instance_id UUID NOT NULL REFERENCES software_instance(instance_id),
    path TEXT NOT NULL,
    role TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    mtime TIMESTAMPTZ NOT NULL,
    inode BIGINT NOT NULL,
    PRIMARY KEY (instance_id, path)
);

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0006_software_fingerprint')
ON CONFLICT (version) DO NOTHING;
