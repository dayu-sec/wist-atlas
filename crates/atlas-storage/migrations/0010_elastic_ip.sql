-- 0010: 弹性公网 IP（EIP）作为独立资产
--
-- 为什么不是主机的一个字段：云上可以先申请、后绑定、解绑后仍保留，换一台机器绑同一个
-- 地址；反过来机器下线了 EIP 还在。独立生命周期 + 独立计费（闲置也计费）+ 独立归集
-- （属于账号/项目，不属于某台机）—— 这三点都要求它是自己的对象。
--
-- 与实例自带公网 IP 的区别：后者没有独立地址 ID、跟实例同生共死，留在主机的观测属性里
-- （`CLOUD_HOST.public_ips`），不进这张表。
--
-- 绑定只存**当前**状态（`bound_*`）。历史绑定要单独建表，等有消费方再做。

CREATE TABLE IF NOT EXISTS elastic_ip (
    elastic_ip_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    environment_id UUID,
    -- 云侧地址 ID（腾讯云 `AddressId`）：重新绑定 / 换机器都不变。
    external_ref TEXT NOT NULL,
    address TEXT NOT NULL,
    status TEXT NOT NULL,
    charge_type TEXT,
    bandwidth_mbps BIGINT,
    cloud_account TEXT,
    bound_target_kind TEXT,
    bound_host_id UUID,
    bound_external_target_ref TEXT,
    bound_since TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    last_inventory_at TIMESTAMPTZ NOT NULL,
    stale_at TIMESTAMPTZ,
    miss_count BIGINT NOT NULL DEFAULT 0,
    ingest_scope TEXT,
    UNIQUE (tenant_id, external_ref)
);

-- 退役清扫的主路径：本账号、仍存活。
CREATE INDEX IF NOT EXISTS idx_elastic_ip_account_live
    ON elastic_ip(tenant_id, cloud_account)
    WHERE cloud_account IS NOT NULL AND stale_at IS NULL;

-- 按主机反查它现在挂着哪些公网地址。
CREATE INDEX IF NOT EXISTS idx_elastic_ip_bound_host
    ON elastic_ip(bound_host_id)
    WHERE bound_host_id IS NOT NULL;

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0010_elastic_ip')
ON CONFLICT (version) DO NOTHING;
