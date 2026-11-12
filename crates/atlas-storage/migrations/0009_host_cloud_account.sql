-- 0009: 主机补云端账号归属（退役判定的作用域）
--
-- 为什么需要它：主机退役只能由「本账号的完整快照」判定，而这个目标集合不能靠
-- `ingest_scope` 找 —— 后者是「最后谁写过」，一台装了 agentd 的云主机每 5 分钟
-- 就被端侧覆盖一次，云侧那轮永远扫不到它（要么永不退役，要么被迫跨账号误扫）。
--
-- `cloud_account` 是**资产属性**（`vendor:account`，过渡期用采集器声明的账号键），
-- 由云 API 采集器写、不随采集顺序变化。退役判定改成
-- `cloud_account = 本帧声明的账号 AND cloud_instance_id IS NOT NULL`，
-- 于是：
--
--   * 装了 agentd 的云主机照样能被云侧判死（这正是加它的理由）；
--   * IDC 机器没有 cloud_account，云侧永远不碰它；
--   * 多账号租户互不越界。
--
-- 不加唯一索引：一个账号下实例 ID 才唯一，账号键本身天然是一对多。

ALTER TABLE host_inventory
    ADD COLUMN IF NOT EXISTS cloud_account TEXT;

-- 退役清扫的主路径：本账号、仍存活、有云实例 ID 的主机。
CREATE INDEX IF NOT EXISTS idx_host_inventory_cloud_account
    ON host_inventory(tenant_id, cloud_account)
    WHERE cloud_account IS NOT NULL AND stale_at IS NULL;

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0009_host_cloud_account')
ON CONFLICT (version) DO NOTHING;
