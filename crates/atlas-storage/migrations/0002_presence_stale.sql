-- 0002: 快照替换语义（presence）
--
-- 一次 `atlas.in.*` 的 `mode = snapshot` 是一份**完整**快照。它没出现的进程不应该继续
-- 以「存活」的样子留在资产目录里 —— 但也不删行：目录要能回答「上周这台机有哪些进程」。
--
-- 所以加两列，不新建表：
--
--   stale_at      第一次发现它消失的时刻。NULL = 仍存活。保留**首次**消失时间，
--                 后续快照不会把它往后推（否则「什么时候没的」这个问题永远答不了）。
--   ingest_scope  归属的快照作用域 `schema:system:producer:tenant:env`。
--                 没有它就无法回答「哪些进程属于这份快照」—— 只按 host_id 划会把
--                 同一台主机上其它生产者（如容器采集器）报的进程一起标 stale。
--
-- 只改 process_runtime_state。host_inventory / network_segment 的退役是另一类问题
-- （主机消失可能只是一轮探针失败），需要单独的策略决定，不在这一版一起做。

ALTER TABLE process_runtime_state
    ADD COLUMN IF NOT EXISTS stale_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS ingest_scope TEXT;

-- 查询侧的主路径：某台主机的存活进程。
CREATE INDEX IF NOT EXISTS idx_process_runtime_state_host_live
    ON process_runtime_state(host_id)
    WHERE stale_at IS NULL;

-- 按作用域做「本次没出现的」比对。
CREATE INDEX IF NOT EXISTS idx_process_runtime_state_scope_live
    ON process_runtime_state(tenant_id, ingest_scope)
    WHERE stale_at IS NULL;

-- 记一笔，让 `schema_migrations` 说真话（0001 是在自己的 SQL 里插的）。
INSERT INTO schema_migrations(version)
VALUES ('0002_presence_stale')
ON CONFLICT (version) DO NOTHING;
