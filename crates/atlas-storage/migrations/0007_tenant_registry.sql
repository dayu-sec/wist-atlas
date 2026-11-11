-- 租户注册表：atlas 内部租户 UUID ↔ 外部客户号。
--
-- 为什么需要它：
--   1. `tenant_id` 在 22 张表里都是 `uuid` 列（并且进了 10 个主键/唯一键），
--      客户号（`1000000000000000001` / `c-a`）塞不进去；
--   2. 只存 UUID 的话，`psql` 里看到的是一串裸 UUID，排障时无从反查「这是哪个客户」；
--   3. 每客户一个库时，库名在 dump / 搬库 / 合并时会丢，只有库内的一行能自证归属。
--
-- 谁是权威：客户清单（栈渲染的 `cells.json`）在 atlas 启动时**幂等**写入这里；
-- ingest 路径**只读不写** —— 未登记的 `cust_id` 一律 400，不在运行时派生 UUID。
CREATE TABLE IF NOT EXISTS tenant (
    tenant_id  UUID PRIMARY KEY,
    tenant_ref TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0007_tenant_registry')
ON CONFLICT (version) DO NOTHING;
