-- 0012: 域（VPC）自己的地址空间
--
-- 域之前只有名字和云侧 ID。VPC 的 CIDR 是它**最核心的属性**（「这个地址空间有多大」
-- 「两台机是不是同一个 VPC」都靠它），不存就只能每次回云上问。
--
-- 存成单值：现网 VPC 都是单 IPv4 段；多段/双栈等真出现再拆表（与 EIP 历史绑定同一条纪律）。

ALTER TABLE network_domain
    ADD COLUMN IF NOT EXISTS cidr TEXT;

-- 记一笔，让 `schema_migrations` 说真话。
INSERT INTO schema_migrations(version)
VALUES ('0012_network_domain_cidr')
ON CONFLICT (version) DO NOTHING;
