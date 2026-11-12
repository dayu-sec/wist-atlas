#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: &'static str,
    pub sql: &'static str,
}

pub const INITIAL_SCHEMA_VERSION: &str = "0001_initial_schema";
pub const PRESENCE_STALE_VERSION: &str = "0002_presence_stale";
pub const HOST_ARCH_PRESENCE_VERSION: &str = "0003_host_arch_presence";
pub const INSTANCE_ENDPOINT_VERSION: &str = "0004_instance_endpoint";
pub const CONTAINER_RUNTIME_VERSION: &str = "0005_container_runtime";
pub const SOFTWARE_FINGERPRINT_VERSION: &str = "0006_software_fingerprint";
pub const TENANT_REGISTRY_VERSION: &str = "0007_tenant_registry";

pub const INITIAL_SCHEMA_SQL: &str = include_str!("../migrations/0001_initial_schema.sql");
pub const PRESENCE_STALE_SQL: &str = include_str!("../migrations/0002_presence_stale.sql");
pub const HOST_ARCH_PRESENCE_SQL: &str = include_str!("../migrations/0003_host_arch_presence.sql");
pub const INSTANCE_ENDPOINT_SQL: &str = include_str!("../migrations/0004_instance_endpoint.sql");
pub const CONTAINER_RUNTIME_SQL: &str = include_str!("../migrations/0005_container_runtime.sql");
pub const SOFTWARE_FINGERPRINT_SQL: &str =
    include_str!("../migrations/0006_software_fingerprint.sql");
pub const TENANT_REGISTRY_SQL: &str = include_str!("../migrations/0007_tenant_registry.sql");
/// 主机补 `cloud_instance_id`：双来源归并键（agentd 的元数据 vs 云 API 的 `InstanceId`）。
pub const HOST_CLOUD_INSTANCE_ID_VERSION: &str = "0008_host_cloud_instance_id";
pub const HOST_CLOUD_INSTANCE_ID_SQL: &str =
    include_str!("../migrations/0008_host_cloud_instance_id.sql");

/// 主机补 `cloud_account`：退役判定的作用域（按账号，不按最后写入者）。
pub const HOST_CLOUD_ACCOUNT_VERSION: &str = "0009_host_cloud_account";
pub const HOST_CLOUD_ACCOUNT_SQL: &str = include_str!("../migrations/0009_host_cloud_account.sql");

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: INITIAL_SCHEMA_VERSION,
        sql: INITIAL_SCHEMA_SQL,
    },
    Migration {
        version: PRESENCE_STALE_VERSION,
        sql: PRESENCE_STALE_SQL,
    },
    Migration {
        version: HOST_ARCH_PRESENCE_VERSION,
        sql: HOST_ARCH_PRESENCE_SQL,
    },
    Migration {
        version: INSTANCE_ENDPOINT_VERSION,
        sql: INSTANCE_ENDPOINT_SQL,
    },
    Migration {
        version: CONTAINER_RUNTIME_VERSION,
        sql: CONTAINER_RUNTIME_SQL,
    },
    Migration {
        version: SOFTWARE_FINGERPRINT_VERSION,
        sql: SOFTWARE_FINGERPRINT_SQL,
    },
    Migration {
        version: TENANT_REGISTRY_VERSION,
        sql: TENANT_REGISTRY_SQL,
    },
    Migration {
        version: HOST_CLOUD_INSTANCE_ID_VERSION,
        sql: HOST_CLOUD_INSTANCE_ID_SQL,
    },
    Migration {
        version: HOST_CLOUD_ACCOUNT_VERSION,
        sql: HOST_CLOUD_ACCOUNT_SQL,
    },
];

#[cfg(test)]
mod tests {
    use super::INITIAL_SCHEMA_SQL;

    #[test]
    fn cloud_instance_id_migration_is_idempotent_and_tenant_scoped() {
        let sql = super::HOST_CLOUD_INSTANCE_ID_SQL;
        assert!(sql.contains("ADD COLUMN IF NOT EXISTS cloud_instance_id"));
        // 唯一索引必须按租户限定：不同租户各自的云账号下，实例 ID 可能撞。
        assert!(
            sql.contains("ON host_inventory(tenant_id, cloud_instance_id)"),
            "cloud_instance_id 的唯一索引必须带 tenant_id"
        );
        // 迁移要能被重复执行。
        assert!(sql.contains("ON CONFLICT (version) DO NOTHING"));
        // 注册进了迁移表，否则新库建好也没有这一列。
        assert!(
            super::MIGRATIONS
                .iter()
                .any(|migration| migration.version == super::HOST_CLOUD_INSTANCE_ID_VERSION)
        );
    }

    #[test]
    fn cloud_account_migration_is_idempotent_and_scoped_by_tenant() {
        let sql = super::HOST_CLOUD_ACCOUNT_SQL;
        assert!(sql.contains("ADD COLUMN IF NOT EXISTS cloud_account"));
        // 退役清扫按 (tenant, account) 取候选集。
        assert!(
            sql.contains("ON host_inventory(tenant_id, cloud_account)"),
            "cloud_account 的索引必须带 tenant_id"
        );
        // 不加唯一索引：账号键天然是一对多。
        assert!(!sql.contains("UNIQUE"));
        assert!(sql.contains("ON CONFLICT (version) DO NOTHING"));
        assert!(
            super::MIGRATIONS
                .iter()
                .any(|migration| migration.version == super::HOST_CLOUD_ACCOUNT_VERSION)
        );
    }

    #[test]
    fn initial_schema_contains_p0_tables() {
        for table in [
            "host_inventory",
            "network_domain",
            "network_segment",
            "host_net_assoc",
            "host_runtime_state",
            "process_runtime_state",
            "service_instance",
            "runtime_binding",
            "subject",
            "responsibility_assignment",
            "ingest_job",
        ] {
            assert!(
                INITIAL_SCHEMA_SQL.contains(table),
                "initial schema should contain {table}"
            );
        }
    }

    #[test]
    fn initial_schema_records_migration_version() {
        assert!(INITIAL_SCHEMA_SQL.contains("0001_initial_schema"));
    }

    #[test]
    fn presence_migration_adds_both_columns_idempotently() {
        let sql = super::PRESENCE_STALE_SQL;
        for column in ["stale_at", "ingest_scope"] {
            assert!(
                sql.contains(column),
                "presence migration should add {column}"
            );
        }
        // 幂等：迁移可能被重复执行（`schema_migrations` 没写入就跑了一半）。
        assert_eq!(sql.matches("ADD COLUMN IF NOT EXISTS").count(), 2);
    }

    #[test]
    fn presence_migration_stays_off_the_host_tables() {
        // 主机/网段的退役需要单独的策略（一轮探针失败不等于主机没了），
        // 这一版只动 process_runtime_state。
        let sql = super::PRESENCE_STALE_SQL;
        assert!(!sql.contains("ALTER TABLE host_inventory"));
        assert!(!sql.contains("ALTER TABLE network_segment"));
    }

    #[test]
    fn host_presence_migration_adds_arch_and_the_miss_counter() {
        let sql = super::HOST_ARCH_PRESENCE_SQL;
        for column in ["arch", "stale_at", "ingest_scope", "miss_count"] {
            assert!(
                sql.contains(column),
                "host presence migration should add {column}"
            );
        }
        assert_eq!(sql.matches("ADD COLUMN IF NOT EXISTS").count(), 4);
    }

    #[test]
    fn every_migration_records_its_own_version() {
        // `schema_migrations` 是「库里到底应用了哪些」的唯一凭据。0001 是在自己的 SQL 里
        // 插的，后来的迁移漏掉过一次 —— 查库会以为只应用了第一版。
        for migration in super::MIGRATIONS {
            assert!(
                migration.sql.contains(migration.version),
                "{} 没有把自己记进 schema_migrations",
                migration.version
            );
            assert!(
                migration.sql.contains("ON CONFLICT"),
                "{} 的版本插入必须幂等（迁移每次启动都会重放）",
                migration.version
            );
        }
    }

    #[test]
    fn instance_endpoint_migration_keeps_history_and_indexes_the_live_path() {
        let sql = super::INSTANCE_ENDPOINT_SQL;
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS instance_endpoint"));
        // 关闭地址只写 valid_to，不删行：历史地址要查得到。
        assert!(sql.contains("valid_to"));
        assert!(sql.contains("WHERE valid_to IS NULL"));
        // 不冗余归属列：查询侧走 service_instance → runtime_binding。
        assert!(!sql.contains("host_id UUID"));
        assert!(!sql.contains("process_id UUID"));
    }

    #[test]
    fn the_miss_counter_defaults_to_zero_so_legacy_rows_are_live() {
        // 升级前的存量主机没有计数：默认 0 = 存活，不能因为缺失列就把老资产全标退役。
        let sql = super::HOST_ARCH_PRESENCE_SQL;
        assert!(sql.contains("miss_count BIGINT NOT NULL DEFAULT 0"));
    }

    #[test]
    fn software_migration_lands_the_customer_domain_and_the_artifact_dictionary() {
        let sql = super::SOFTWARE_FINGERPRINT_SQL;
        for table in [
            "software_artifact",
            "software_instance",
            "software_fingerprint",
            "software_instance_observation",
            "software_artifact_path",
        ] {
            assert!(
                sql.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
                "software migration should create {table}"
            );
        }
    }

    #[test]
    fn software_migration_keeps_identity_rules_in_the_schema() {
        let sql = super::SOFTWARE_FINGERPRINT_SQL;
        // 一个部署位只能有一行：否则同一份软件每轮采样都可能给不同身份。
        assert!(sql.contains("UNIQUE (host_id, anchor_kind, anchor_value)"));
        // 允许「没有 ID」，不允许「假装有 ID」。
        assert!(sql.contains("software_instance_identity_or_reason"));
        assert!(
            sql.contains("current_artifact_id IS NOT NULL OR identity_missing_reason IS NOT NULL")
        );
    }

    #[test]
    fn software_migration_does_not_resurrect_the_package_view() {
        // `software_installation` 已并入 `software_instance`（2026-11-08）。
        // 它一旦回流，就会出现两套客户域模型、两条 join、两份会漂的事实。
        let sql = super::SOFTWARE_FINGERPRINT_SQL;
        assert!(!sql.contains("CREATE TABLE IF NOT EXISTS software_installation"));
    }
}
