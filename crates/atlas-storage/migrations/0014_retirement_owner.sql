-- 0014: 云账号的退役权归属
--
-- 一个云账号只该由一个采集器声称「账号级完整覆盖」。两个生产者同时报同一账号时，
-- 每一帧都会给缺行 `miss_count + 1`，退役速度直接翻倍（3 轮变 1.5 轮）—— 方向是错的：
-- 采集器抖动一次就可能把还在的机器判死。
--
-- 所以退役权要显式登记：谁先声明谁持有，其他人照常写数据（upsert 是幂等的、安全的），
-- 但不推进退役。持有者长期不出声（见 `RETIREMENT_OWNER_TTL`）才允许接管，避免换采集器
-- 之后永远冻结。
--
-- 为什么按 `cloud_account` 而不是按 `ingest_scope`：后者是「最后谁写过」，一台装了
-- agentd 的云主机每 5 分钟被端侧覆盖一次，拿它当归属会来回翻。

CREATE TABLE IF NOT EXISTS retirement_owner (
    tenant_id UUID NOT NULL,
    cloud_account TEXT NOT NULL,
    -- 生产者标识，与 `IngestEnvelope::source_name`（`system:producer`）同源。
    producer TEXT NOT NULL,
    -- 持有者最近一次行使退役权（= 最近一次声明账号级完整覆盖）的时刻。
    claimed_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id, cloud_account)
);

INSERT INTO schema_migrations(version)
VALUES ('0014_retirement_owner')
ON CONFLICT (version) DO NOTHING;
