#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: &'static str,
    pub sql: &'static str,
}

pub const INITIAL_SCHEMA_VERSION: &str = "0001_initial_schema";
pub const PRESENCE_STALE_VERSION: &str = "0002_presence_stale";
pub const HOST_ARCH_PRESENCE_VERSION: &str = "0003_host_arch_presence";

pub const INITIAL_SCHEMA_SQL: &str = include_str!("../migrations/0001_initial_schema.sql");
pub const PRESENCE_STALE_SQL: &str = include_str!("../migrations/0002_presence_stale.sql");
pub const HOST_ARCH_PRESENCE_SQL: &str = include_str!("../migrations/0003_host_arch_presence.sql");

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
];

#[cfg(test)]
mod tests {
    use super::INITIAL_SCHEMA_SQL;

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
    fn the_miss_counter_defaults_to_zero_so_legacy_rows_are_live() {
        // 升级前的存量主机没有计数：默认 0 = 存活，不能因为缺失列就把老资产全标退役。
        let sql = super::HOST_ARCH_PRESENCE_SQL;
        assert!(sql.contains("miss_count BIGINT NOT NULL DEFAULT 0"));
    }
}
