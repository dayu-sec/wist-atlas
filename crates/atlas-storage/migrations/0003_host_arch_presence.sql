-- 0003: 主机架构 + 主机退役（presence）
--
-- 两件事共用一张表的迁移，因为都是 `host_inventory` 的列：
--
-- 1. `arch` —— CPU 架构。包/二进制匹配、漏洞影响面判断都要它，没有它只能看到
--    「一台 macOS 26.6.2 的机器」，看不出是 arm64 还是 x86_64。
--
-- 2. 主机退役。与 0002 里进程的做法**不同**，这里不能拿「一份快照没出现」当判据：
--    进程 churn 是分钟级常态，而主机消失可能只是一轮探针失败 / 主机短暂离线。
--    误标一台活主机的代价远大于晚几轮才标死，所以用**连续缺失计数**：
--
--      miss_count  连续多少份完整快照里没出现它。出现即清零。
--      stale_at    首次判定退役的时刻（达到阈值才写，之后不再推后）。
--
--    阈值由 `HOST_STALE_AFTER_MISSES` 定（atlas-api），默认 3 份。

ALTER TABLE host_inventory
    ADD COLUMN IF NOT EXISTS arch TEXT,
    ADD COLUMN IF NOT EXISTS stale_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS ingest_scope TEXT,
    ADD COLUMN IF NOT EXISTS miss_count BIGINT NOT NULL DEFAULT 0;

-- 查询侧主路径：存活主机。
CREATE INDEX IF NOT EXISTS idx_host_inventory_live
    ON host_inventory(tenant_id, host_name)
    WHERE stale_at IS NULL;

-- 按作用域做「本次没出现的」比对。
CREATE INDEX IF NOT EXISTS idx_host_inventory_scope_live
    ON host_inventory(tenant_id, ingest_scope)
    WHERE stale_at IS NULL;
