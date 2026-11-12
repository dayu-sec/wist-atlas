-- 0013: 网络对象也要能退役
--
-- 只有主机和 EIP 有 stale/miss 字段，子网和 VPC 没有 —— 云上删了子网，atlas 里会留到
-- 天荒地老，而且看起来跟还在的一样。
--
-- 判据与主机完全一致（**账号级完整快照 + 连续 N 轮**），所以同样需要 cloud_account
-- 来圈定目标集，ingest_scope 用于诊断（这一行最后一次是谁写的）。

ALTER TABLE network_segment
    ADD COLUMN IF NOT EXISTS cloud_account TEXT,
    ADD COLUMN IF NOT EXISTS stale_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS miss_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS ingest_scope TEXT;

ALTER TABLE network_domain
    ADD COLUMN IF NOT EXISTS cloud_account TEXT,
    ADD COLUMN IF NOT EXISTS stale_at TIMESTAMPTZ,
    -- 域不单独上报，`miss_count` 恒为 0；域的退役靠「域下还有没有活段」推导。
    -- 留着这一列只是为了两个表结构对称、读路径同一套代码。
    ADD COLUMN IF NOT EXISTS miss_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS ingest_scope TEXT;

-- 退役清扫的主路径：本账号、仍存活。
CREATE INDEX IF NOT EXISTS idx_network_segment_account_live
    ON network_segment(tenant_id, cloud_account)
    WHERE cloud_account IS NOT NULL AND stale_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_network_domain_account_live
    ON network_domain(tenant_id, cloud_account)
    WHERE cloud_account IS NOT NULL AND stale_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('0013_network_presence')
ON CONFLICT (version) DO NOTHING;
