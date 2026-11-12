-- 0008: 主机补云实例 ID（跨来源归并键）
--
-- 为什么需要它：一台云主机会被两个来源报到 —— agentd（本机发现）和云 API
-- （wist-harvest）。两者给的外部 ID 完全不同（本机 machine_id vs InstanceId），
-- 没有这一列就只能在拓扑里看到**两个节点**。
--
-- 为什么不复用 machine_id：machine_id 是**机器自证**（/etc/machine-id），
-- cloud_instance_id 是**云平台给的**。一台云主机两个都有；混成一个字段会让
-- 「云上换了实例」与「机器重装了系统」看起来是同一件事。
--
-- 唯一性按租户限定：不同租户各自云账号下的实例 ID 理论上可能撞。
-- 全局唯一索引（如 machine_id 那样）在这里是错的。

ALTER TABLE host_inventory
    ADD COLUMN IF NOT EXISTS cloud_instance_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_host_inventory_cloud_instance_id
    ON host_inventory(tenant_id, cloud_instance_id)
    WHERE cloud_instance_id IS NOT NULL;

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0008_host_cloud_instance_id')
ON CONFLICT (version) DO NOTHING;
