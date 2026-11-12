//! 静态守卫：PG 语句里的占位符必须带显式类型转换。
//!
//! ## 它守的是什么
//!
//! 存储层统一把参数当**字符串**绑定（`&[String]` → `&dyn ToSql`）。于是
//! `WHERE network_domain_id = $1` 会被 tokio-postgres 按**列类型**推断成 `uuid`，
//! 然后拒掉字符串参数：
//!
//! ```text
//! query postgres sql: error serializing parameter 0
//! ```
//!
//! 真账号 smoke 就是这么炸的：`get_network_domain` 只在「域下子网全被删」时才走到，
//! 单测和 `--ignored` 真库用例都没覆盖那条路径，于是这条语句带着 bug 活到了线上。
//! 同一个毛病当时还有 5 处（`subject` / `software_artifact` /
//! `responsibility_assignment` 的按 ID 查询，以及两处裸 `LIMIT $n`）。
//!
//! ## 怎么守
//!
//! 不靠人眼记规矩：把 DDL 里的列类型读出来，再把 SQL 里的 `列 = $n`、`LIMIT $n`、
//! `OFFSET $n` 找出来。凡是**非 TEXT 列**的占位符没有 `NULLIF(...)` / `COALESCE(NULLIF(...))`
//! 或显式 `::` 转换，就报错。这样「新写一条 SQL 忘了转换」在 `cargo test` 阶段就红，
//! 不需要真库。
//!
//! 只覆盖**本仓自己写的 SQL**：`sql.rs` 的常量 + 两个 store 目录里的内联语句。

use std::collections::BTreeMap;

/// 参与扫描的 SQL 来源。加新文件要加到这里，否则等于没扫。
const SQL_SOURCES: &[(&str, &str)] = &[
    ("sql.rs", include_str!("sql.rs")),
    (
        "sync_store/catalog.rs",
        include_str!("sync_store/catalog.rs"),
    ),
    (
        "sync_store/governance.rs",
        include_str!("sync_store/governance.rs"),
    ),
    ("sync_store/ingest.rs", include_str!("sync_store/ingest.rs")),
    (
        "sync_store/runtime.rs",
        include_str!("sync_store/runtime.rs"),
    ),
    (
        "live_async/live_catalog.rs",
        include_str!("live_async/live_catalog.rs"),
    ),
    (
        "live_async/live_governance.rs",
        include_str!("live_async/live_governance.rs"),
    ),
    (
        "live_async/live_ingest.rs",
        include_str!("live_async/live_ingest.rs"),
    ),
    (
        "live_async/live_runtime.rs",
        include_str!("live_async/live_runtime.rs"),
    ),
];

/// 建表 DDL（列名 → 类型）。
const DDL_SOURCES: &[&str] = &[
    include_str!("../../migrations/0001_initial_schema.sql"),
    include_str!("../../migrations/0002_presence_stale.sql"),
    include_str!("../../migrations/0003_host_arch_presence.sql"),
    include_str!("../../migrations/0004_instance_endpoint.sql"),
    include_str!("../../migrations/0005_container_runtime.sql"),
    include_str!("../../migrations/0006_software_fingerprint.sql"),
    include_str!("../../migrations/0007_tenant_registry.sql"),
    include_str!("../../migrations/0008_host_cloud_instance_id.sql"),
    include_str!("../../migrations/0009_host_cloud_account.sql"),
    include_str!("../../migrations/0010_elastic_ip.sql"),
    include_str!("../../migrations/0011_network_identity.sql"),
    include_str!("../../migrations/0012_network_domain_cidr.sql"),
    include_str!("../../migrations/0013_network_presence.sql"),
    include_str!("../../migrations/0014_retirement_owner.sql"),
];

/// 参数是字符串绑定，只有 TEXT 类列能直接比较；其余都得显式转换。
fn is_text_type(sql_type: &str) -> bool {
    let upper = sql_type.trim().to_ascii_uppercase();
    upper.starts_with("TEXT") || upper.starts_with("VARCHAR") || upper.starts_with("CHAR")
}

/// 从 DDL 里抽 `列名 → 类型`。
///
/// 只认行首的 `列名 类型`（`CREATE TABLE` 块里的声明就是这样），
/// 以及 `ALTER TABLE ... ADD COLUMN IF NOT EXISTS 列名 类型`。
fn column_types() -> BTreeMap<String, String> {
    let mut types: BTreeMap<String, String> = BTreeMap::new();
    for ddl in DDL_SOURCES {
        for line in ddl.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }
            let words: Vec<&str> = trimmed.split_whitespace().collect();
            // `ALTER TABLE x ADD COLUMN IF NOT EXISTS col TYPE`
            let (column, sql_type) = if let Some(at) = words
                .iter()
                .position(|word| word.eq_ignore_ascii_case("EXISTS"))
            {
                match (words.get(at + 1), words.get(at + 2)) {
                    (Some(column), Some(sql_type)) => (*column, *sql_type),
                    _ => continue,
                }
            } else {
                match (words.first(), words.get(1)) {
                    (Some(column), Some(sql_type)) => (*column, *sql_type),
                    _ => continue,
                }
            };
            // 列名必须是裸标识符；`CREATE`/`PRIMARY`/`UNIQUE` 之类自然被排除。
            if !column
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                continue;
            }
            if column.is_empty() || column == "create" {
                continue;
            }
            types
                .entry(column.to_string())
                .or_insert_with(|| sql_type.trim_end_matches(',').to_string());
        }
    }
    types
}

/// 占位符是不是被包在能决定类型的表达式里。
///
/// - `before`：`$` 之前的文本；`after`：从 `$` 起。
fn placeholder_is_cast(before: &str, after: &str) -> bool {
    let tail = before.trim_end();
    if tail.ends_with("NULLIF(") || tail.ends_with("COALESCE(") {
        return true;
    }
    // `$1::uuid`（显式转换直接跟在占位符后面）
    let rest = after.trim_start();
    match rest.find(|c: char| c != '$' && !c.is_ascii_digit()) {
        Some(at) => rest[at..].starts_with("::"),
        None => false,
    }
}

/// 一行 SQL 里所有「非 TEXT 列的占位符没转换」的问题。
fn violations_in_line(file: &str, line_no: usize, code: &str) -> Vec<String> {
    let types = column_types();
    let mut out = Vec::new();

    // `列 = $n`
    let chars: Vec<char> = code.chars().collect();
    for (at, ch) in chars.iter().enumerate() {
        if *ch != '=' {
            continue;
        }
        let prev = at.checked_sub(1).map(|i| chars[i]);
        let next = chars.get(at + 1).copied();
        if matches!(prev, Some('>') | Some('<') | Some('!') | Some('=')) || next == Some('=') {
            continue;
        }
        let rest: String = chars[at + 1..].iter().collect();
        if !rest.trim_start().starts_with('$') {
            continue;
        }
        let before: String = chars[..at].iter().collect();
        let column = before
            .trim_end()
            .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .next()
            .unwrap_or_default()
            .to_string();
        let Some(sql_type) = types.get(&column) else {
            continue;
        };
        if is_text_type(sql_type) || placeholder_is_cast(&before, &rest) {
            continue;
        }
        out.push(format!(
            "{file}:{line_no}: `{column}` 是 {sql_type}，但 `= $n` 没有 NULLIF / 显式转换 \
             （字符串参数会被当成 {sql_type} 拒掉）"
        ));
    }

    // `LIMIT $n` / `OFFSET $n`：PG 推断成 bigint，同样拒字符串。
    for keyword in ["LIMIT", "OFFSET"] {
        let mut search = code;
        while let Some(at) = search.find(keyword) {
            let tail = &search[at + keyword.len()..];
            let trimmed = tail.trim_start();
            if trimmed.starts_with('$') {
                let before_len = tail.len() - trimmed.len();
                let (before, after) = tail.split_at(before_len);
                if !placeholder_is_cast(before, after) {
                    out.push(format!(
                        "{file}:{line_no}: `{keyword} $n` 没有 NULLIF / 显式转换（PG 会推断成 bigint）"
                    ));
                }
            }
            search = &search[at + keyword.len()..];
        }
    }

    out
}

#[test]
fn every_non_text_placeholder_carries_an_explicit_cast() {
    let types = column_types();
    assert!(
        types.contains_key("network_domain_id") && types.contains_key("tenant_id"),
        "DDL 解析没拿到已知列，守卫会变成空转"
    );

    let mut violations: Vec<String> = Vec::new();
    for (file, source) in SQL_SOURCES {
        for (line_no, line) in source.lines().enumerate() {
            // 跳过注释行：注释里写的例子不该被当成语句。
            let code = match line.find("//") {
                Some(at) => &line[..at],
                None => line,
            };
            violations.extend(violations_in_line(file, line_no + 1, code));
        }
    }

    assert!(
        violations.is_empty(),
        "SQL 占位符类型转换缺失（真库上会报 `error serializing parameter N`）：\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_guard_catches_the_original_bug_and_its_siblings() {
    // 守卫自己也要有守卫：当年那几条语句必须被拦下，修好的不能误报。
    let bad = "SELECT network_domain_id FROM network_domain WHERE network_domain_id = $1";
    let good = "SELECT network_domain_id FROM network_domain \
                WHERE network_domain_id = NULLIF($1, '')::uuid";
    let bad_limit = "SELECT subject_id FROM subject WHERE tenant_id = NULLIF($1,'')::uuid LIMIT $2";
    let bad_subject = "SELECT subject_id FROM subject WHERE tenant_id = $1 ORDER BY subject_id ASC LIMIT NULLIF($2, '')::int4";

    let problems = |sql: &str| violations_in_line("probe", 1, sql).len();
    assert_eq!(problems(bad), 1, "原始的 `= $1` 必须被拦下");
    assert_eq!(problems(good), 0, "加过 NULLIF 的不能误报");
    assert_eq!(problems(bad_limit), 1, "裸 LIMIT 必须被拦下");
    assert_eq!(problems(bad_subject), 1, "tenant_id 的裸比较必须被拦下");
}

#[test]
fn text_columns_do_not_need_a_cast() {
    // `WHERE ingest_id = $1`（TEXT）是合法的：PG 直接推断成 text。
    assert!(
        violations_in_line("probe", 1, "SELECT x FROM ingest_job WHERE ingest_id = $1").is_empty()
    );
}
