-- 容器运行时（`ContainerRuntime`）。
--
-- 为什么要有这张表：主机资产里「跑着什么进程」已经有了，但容器是另一层抽象 ——
-- 同一个镜像可以在多台机器上跑 N 份，容器 id 是运行时自己的身份，和 pid 没有稳定关系。
-- 采集侧（agentd 的 container discovery）已经能给出 container.id / name / runtime / k8s 线索，
-- 这里把观测事实落下来。
--
-- 与 `process_runtime_state` 一样用 presence 语义：
--   `stale_at` 非空 = 这个容器在某轮完整快照里消失过，行保留以备回溯。
--   `ingest_scope` 记录是哪份快照作用域看到的，清扫只比对同作用域的行 ——
--   一台主机可能被多个生产者看着，按 host 清会把别人的容器一起清掉。
CREATE TABLE IF NOT EXISTS container_runtime (
    container_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    pod_id UUID,
    host_id UUID NOT NULL REFERENCES host_inventory(host_id),
    runtime_type TEXT NOT NULL,
    runtime_namespace TEXT,
    container_name TEXT,
    image_ref TEXT,
    external_ref TEXT,
    started_at TIMESTAMPTZ,
    last_seen_at TIMESTAMPTZ NOT NULL,
    stale_at TIMESTAMPTZ,
    ingest_scope TEXT,
    -- K8s 线索（采集侧原值）。pod 目录还没有，先存下来。
    pod_uid TEXT,
    pod_name TEXT,
    k8s_namespace TEXT
);

CREATE INDEX IF NOT EXISTS idx_container_runtime_host_id
    ON container_runtime(host_id);

CREATE INDEX IF NOT EXISTS idx_container_runtime_external_ref
    ON container_runtime(tenant_id, external_ref)
    WHERE external_ref IS NOT NULL;

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0005_container_runtime')
ON CONFLICT (version) DO NOTHING;
