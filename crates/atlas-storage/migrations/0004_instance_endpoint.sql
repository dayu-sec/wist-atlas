-- 0004: 实例运行地址（InstEp）
--
-- `ServiceInstance` 回答「哪个服务的哪次运行」，但没法回答「它当时监听在哪个地址」。
-- 实例地址是运行态里变化最快的一类事实（Pod 重建即换 IP，进程重启可能换端口），
-- 所以单独成表，而不是给 `service_instance` 加一列：
--
--   * 一个实例可以同时监听多个地址（多个端口 / 多张网卡），一列装不下。
--   * 地址有自己的生命周期。沿用 runtime_binding 的做法，用 `valid_to` 关闭旧行而不删行，
--     否则「上周这个实例在哪」永远答不了。
--
-- 不冗余 host_id / process_id：查询侧是 `service_instance` → `runtime_binding` 找到实例，
-- 再从实例查地址。多存一份归属关系只会多一份可能过期的数据。
--
-- tenant_id 虽然能顺着 inst_id 关联出来，仍然落列：租户是最外层的过滤条件，
-- 每次查询都多一次 join 不值得，而且写入方本来就知道租户。
--
-- 注意这里**不加** `runtime_binding` 那种「同一归属只能有一条未关闭记录」的唯一索引：
-- 同一实例的多个监听地址是常态，唯一性只能落在 endpoint_id 上。
-- 生产侧的重复写入由 `source` + `address` + `port` 判定，交由调用方决定是否先 close。

CREATE TABLE IF NOT EXISTS instance_endpoint (
    endpoint_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    inst_id UUID NOT NULL REFERENCES service_instance(instance_id),
    address TEXT NOT NULL,
    port INTEGER NOT NULL,
    protocol TEXT NOT NULL,
    source TEXT NOT NULL,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CHECK (valid_to IS NULL OR valid_to > valid_from)
);

-- 按实例列出地址（含历史）。
CREATE INDEX IF NOT EXISTS idx_instance_endpoint_inst_id ON instance_endpoint(inst_id);

-- 查询主路径只看「当前地址」，历史行不该进索引。
CREATE INDEX IF NOT EXISTS idx_instance_endpoint_live ON instance_endpoint(inst_id) WHERE valid_to IS NULL;

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0004_instance_endpoint')
ON CONFLICT (version) DO NOTHING;
