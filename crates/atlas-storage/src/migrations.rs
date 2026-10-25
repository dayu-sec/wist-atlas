#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: &'static str,
    pub sql: &'static str,
}

pub const INITIAL_SCHEMA_VERSION: &str = "0001_initial_schema";
pub const PRESENCE_STALE_VERSION: &str = "0002_presence_stale";

pub const INITIAL_SCHEMA_SQL: &str = include_str!("../migrations/0001_initial_schema.sql");
pub const PRESENCE_STALE_SQL: &str = include_str!("../migrations/0002_presence_stale.sql");

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: INITIAL_SCHEMA_VERSION,
        sql: INITIAL_SCHEMA_SQL,
    },
    Migration {
        version: PRESENCE_STALE_VERSION,
        sql: PRESENCE_STALE_SQL,
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
}
