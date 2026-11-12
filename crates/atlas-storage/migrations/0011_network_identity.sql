-- 0011: 网络对象的身份修正（真账号数据逼出来的）
--
-- 三条约束在建库时看着合理，拿真云账号一跑全是错的：
--
-- 1. `network_domain UNIQUE (tenant_id, name)`
--    腾讯云每个地域都送建一个 `Default-VPC`，10 个地域就是 10 个同名 VPC。
--    名字在云上是**用户自定义、不保证唯一**的，拿它当身份直接撞唯一约束。
--
-- 2. `network_segment UNIQUE (tenant_id, name)`
--    同理：子网名在不同 VPC 之间可以重复（`subnet-a` 到处都是）。
--
-- 3. `idx_network_segment_cidr UNIQUE (tenant_id, cidr)`
--    不同 VPC 的 CIDR **可以重叠**（prod 与 dev 各一层 10.0.0.0/16 是常态），
--    而且 CIDR 改了就是另一行，跟"段"这个对象不是一回事。
--
-- 身份改成云侧 ID（`external_ref`）+ 我们自己派生的主键：
-- 主键保证一行一个对象，`(tenant_id, external_ref)` 保证同一朵云上同一个对象不会裂成两行。
-- CIDR 退化成普通索引（CIDR 包含匹配要用）。

ALTER TABLE network_domain
    DROP CONSTRAINT IF EXISTS network_domain_tenant_id_name_key;
ALTER TABLE network_domain
    ADD COLUMN IF NOT EXISTS external_ref TEXT;

ALTER TABLE network_segment
    DROP CONSTRAINT IF EXISTS network_segment_tenant_id_name_key;
ALTER TABLE network_segment
    ADD COLUMN IF NOT EXISTS external_ref TEXT;

DROP INDEX IF EXISTS idx_network_segment_cidr;
CREATE INDEX IF NOT EXISTS idx_network_segment_cidr
    ON network_segment(tenant_id, cidr)
    WHERE cidr IS NOT NULL;

-- 云侧 ID 在租户内唯一；端侧没有云 ID 的行（NULL）不受约束。
CREATE UNIQUE INDEX IF NOT EXISTS idx_network_domain_external_ref
    ON network_domain(tenant_id, external_ref)
    WHERE external_ref IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_network_segment_external_ref
    ON network_segment(tenant_id, external_ref)
    WHERE external_ref IS NOT NULL;

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0011_network_identity')
ON CONFLICT (version) DO NOTHING;
