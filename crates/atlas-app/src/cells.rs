//! 多客户 cells：客户清单 → `(store, 租户)` 的路由。
//!
//! 设计依据：`wist-field-stack` 的 `docs/multi-customer.md` §5.7 与 `docs/atlas-m1.7-tasks.md`。
//!
//! 三句话概括：
//!
//! 1. **一个进程连多个库**：每客户一个 atlas 库（共享同一份计算），租户 UUID 在
//!    「登记那一刻」生成并固化，运行时不派生；
//! 2. **清单是权威**：`WIST_ATLAS_CELLS_FILE` 指向的 JSON 由栈渲染，启动时把
//!    `(tenant_id, cust_id)` 幂等写进各库的 `tenant` 表（让库能自证归属）；
//! 3. **未登记一律拒绝**：ingest 收到清单外的 `cust_id` 直接 400 —— 不静默派生、
//!    不落默认库，否则就是「客户 B 的拓扑写进了默认库」。
//!
//! 清单为空 / 没配 `WIST_ATLAS_CELLS_FILE` = **不能再启动**（0.5.0 起）：
//! 默认库已经去掉 —— 每个客户一个库，没有「先落进来再说」的地方。
//! 单客户站点也要登记一条 cell（与 `wist-evidence` 同一条纪律）。

use std::collections::{HashMap, HashSet};

use atlas_domain::TenantId;
use orion_error::conversion::{ConvErr, ToStructError};
use serde::Deserialize;
use uuid::Uuid;

use crate::{AppReason, AppResult, TopologyMonolith};

/// 客户清单文件路径（栈渲染后挂进容器）。
pub(crate) const CELLS_FILE_ENV: &str = "WIST_ATLAS_CELLS_FILE";
/// 各客户库共用的 PG 口令：cells 清单里只放 host/user/database，口令走环境变量，
/// 这样 `configs/` 里的清单可以 0644 可读，不必再发明一套密钥展开机制。
pub(crate) const CELLS_PASSWORD_ENV: &str = "WIST_ATLAS_PG_PASSWORD";

const CUST_ID_MAX_LEN: usize = 64;

/// 清单文件形状。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CellsFile {
    /// 单客户站点的兜底：值是 `cust_id`（不是 UUID）。多客户站点留空。
    #[serde(default)]
    pub default_tenant: Option<String>,
    #[serde(default)]
    pub cells: Vec<CellConfig>,
}

/// 一个客户 = 一个库 + 一个**固化的**租户 UUID。
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CellConfig {
    /// 与写入 body 的 `cust_id`、BFF 的 `?tenant=` 完全一致。
    pub cust_id: String,
    /// 注册时生成的 UUID；atlas **只读不改**。
    pub tenant_id: Uuid,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    /// 覆盖项：特殊 TLS / 参数场景直接给完整连接串。
    #[serde(default)]
    pub database_url: Option<String>,
}

impl CellConfig {
    fn trimmed_cust_id(&self) -> &str {
        self.cust_id.trim()
    }

    /// 拼连接串：`database_url` 优先，否则 `host` + 共用口令。
    fn database_url(&self, password: Option<&str>) -> Result<String, String> {
        if let Some(url) = self
            .database_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
        {
            return Ok(url.to_string());
        }
        let cust_id = self.trimmed_cust_id();
        let host = self
            .host
            .as_deref()
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .ok_or_else(|| {
                format!("cells: cust_id `{cust_id}` 既没有 database_url，也没有 host")
            })?;
        let user = self
            .user
            .as_deref()
            .map(str::trim)
            .filter(|user| !user.is_empty())
            .unwrap_or("atlas");
        let database = self
            .database
            .as_deref()
            .map(str::trim)
            .filter(|database| !database.is_empty())
            .unwrap_or("wist_atlas");
        let password = password.ok_or_else(|| {
            format!("cells: cust_id `{cust_id}` 需要环境变量 {CELLS_PASSWORD_ENV} 才能拼连接串")
        })?;
        // 口令按原样拼进 URL（与栈里其它服务的做法一致）：栈生成的口令是 URL 安全字符。
        Ok(format!(
            "postgres://{user}:{password}@{host}:5432/{database}"
        ))
    }
}

impl CellsFile {
    /// 启动期硬校验：重复客户 / 重复租户 / 非法客户号一律拒启动。
    fn validate(&self) -> Result<(), String> {
        let mut cust_ids: HashSet<&str> = HashSet::new();
        let mut tenant_ids: HashSet<Uuid> = HashSet::new();
        for cell in &self.cells {
            let cust_id = cell.trimmed_cust_id();
            if cust_id.is_empty() {
                return Err("cells: cust_id 不能为空".to_string());
            }
            if cust_id.len() > CUST_ID_MAX_LEN {
                return Err(format!(
                    "cells: cust_id `{cust_id}` 太长（<= {CUST_ID_MAX_LEN}）"
                ));
            }
            if !cust_id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
            {
                return Err(format!("cells: cust_id `{cust_id}` 只允许 [A-Za-z0-9_.-]"));
            }
            if !cust_ids.insert(cust_id) {
                return Err(format!("cells: cust_id `{cust_id}` 重复"));
            }
            if !tenant_ids.insert(cell.tenant_id) {
                return Err(format!(
                    "cells: tenant_id `{}`（cust_id `{cust_id}`）与别的客户重复",
                    cell.tenant_id
                ));
            }
        }
        // 有客户清单时，默认客户必须是其中一个；清单为空时它只是给"单客户站点"留的字段，
        // 交给老路径（`WIST_ATLAS_DEFAULT_TENANT`），这里不校验。
        if !self.cells.is_empty()
            && let Some(default) = self.default_tenant.as_deref().map(str::trim)
            && !default.is_empty()
            && !cust_ids.contains(default)
        {
            return Err(format!(
                "cells: default_tenant `{default}` 不在 cells 清单里"
            ));
        }
        Ok(())
    }
}

/// 一个已连上的客户库。
#[derive(Clone)]
pub(crate) struct RoutedCell {
    pub app: TopologyMonolith,
    pub tenant: TenantId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolveError {
    /// 多客户站点没带 `?tenant=`（读：400 `tenant_required`）。
    Required,
    /// 带了一个不在清单里的租户（读：403 `tenant_not_allowed`；写：400 `tenant_unregistered`）。
    Unknown,
}

#[derive(Clone)]
pub(crate) struct CellRouter {
    default_tenant: Option<String>,
    cells: HashMap<String, RoutedCell>,
    /// 保持清单顺序：日志与 `/healthz` 输出稳定，便于比对。
    order: Vec<String>,
}

impl CellRouter {
    /// 按环境变量装配。
    ///
    /// `Ok(None)` = 没配清单 / 清单为空。**生产入口（`serve`）把它当错误**：
    /// 0.5.0 起没有默认库，一个客户都没有的 atlas 没有可服务的数据。
    /// 保留 `Option` 是为了让「客户清单 → 路由」这一步本身可单测（不依赖进程启动）。
    pub(crate) async fn from_env() -> AppResult<Option<Self>> {
        let Some(path) = std::env::var(CELLS_FILE_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };

        let text = std::fs::read_to_string(&path).map_err(|err| {
            AppReason::InvalidArgs
                .to_err()
                .with_detail(format!("read {CELLS_FILE_ENV} {path}: {err}"))
        })?;
        let file: CellsFile = serde_json::from_str(&text).map_err(|err| {
            AppReason::InvalidArgs
                .to_err()
                .with_detail(format!("parse {CELLS_FILE_ENV} {path}: {err}"))
        })?;
        file.validate().map_err(|err| {
            AppReason::InvalidArgs
                .to_err()
                .with_detail(format!("{CELLS_FILE_ENV} {path}: {err}"))
        })?;

        if file.cells.is_empty() {
            // 没有客户：不在这里报错，交给调用方决定（`serve` 会拒绝启动）。
            return Ok(None);
        }

        Self::build(file).await.map(Some)
    }

    async fn build(file: CellsFile) -> AppResult<Self> {
        let password = std::env::var(CELLS_PASSWORD_ENV).ok();
        let mut cells = HashMap::new();
        let mut order = Vec::new();

        for cell in &file.cells {
            let cust_id = cell.trimmed_cust_id().to_string();
            let url = cell.database_url(password.as_deref()).map_err(|err| {
                AppReason::InvalidArgs
                    .to_err()
                    .with_detail(format!("{CELLS_FILE_ENV}: {err}"))
            })?;
            let app = TopologyMonolith::new_postgres_live(url)
                .await
                .map_err(|err| {
                    AppReason::InputLoadFailed
                        .to_err()
                        .with_detail(format!("connect cell `{cust_id}`: {err}"))
                })?;
            // 播种：让「库」成为租户映射的权威（掉清单也能从库里反查）。
            if let TopologyMonolith::PostgresLive(store) = &app {
                store
                    .upsert_tenant_async(cell.tenant_id, &cust_id)
                    .await
                    .conv_err()?;
            }
            eprintln!(
                "atlas cells: cust_id={cust_id} tenant_id={}",
                cell.tenant_id
            );
            order.push(cust_id.clone());
            cells.insert(
                cust_id,
                RoutedCell {
                    app,
                    tenant: TenantId(cell.tenant_id),
                },
            );
        }

        let default_tenant = default_tenant_of(&file);
        eprintln!(
            "atlas cells: {} customer database(s) configured{}",
            order.len(),
            match default_tenant.as_deref() {
                Some(default) => format!(", default_tenant={default}"),
                None => String::new(),
            }
        );
        Ok(Self::from_parts(default_tenant, order, cells))
    }

    /// 直接给已连好的 store（测试用；也留给将来“进程内共享 store”的场景）。
    pub(crate) fn from_parts(
        default_tenant: Option<String>,
        order: Vec<String>,
        cells: HashMap<String, RoutedCell>,
    ) -> Self {
        Self {
            default_tenant,
            cells,
            order,
        }
    }

    /// 选库。规则与 BFF 的 `resolve_tenant` 同口径，只多一条「单 cell 自动选中」。
    pub(crate) fn resolve(&self, requested: Option<&str>) -> Result<&RoutedCell, ResolveError> {
        let requested = requested.map(str::trim).filter(|value| !value.is_empty());
        if let Some(want) = requested {
            return self.cells.get(want).ok_or(ResolveError::Unknown);
        }
        if self.order.len() == 1
            && let Some(only) = self.order.first().and_then(|cust| self.cells.get(cust))
        {
            return Ok(only);
        }
        if let Some(default) = self.default_tenant.as_deref()
            && let Some(cell) = self.cells.get(default)
        {
            return Ok(cell);
        }
        Err(ResolveError::Required)
    }

    /// 按客户号遍历已连好的库（清单顺序）。
    ///
    /// 只读：给写探针用 —— 探针需要「每个客户一个目标」，且它写的库必须与
    /// HTTP 写路径选中的是同一个，所以只能从路由本身取，不能另编一份。
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &RoutedCell)> {
        self.order
            .iter()
            .filter_map(|cust_id| self.cells.get(cust_id).map(|cell| (cust_id.as_str(), cell)))
    }

    /// `/healthz` 用：**不输出连接串**，只报客户与租户身份。
    pub(crate) fn describe(&self) -> Vec<(String, Uuid)> {
        self.order
            .iter()
            .filter_map(|cust| {
                self.cells
                    .get(cust)
                    .map(|cell| (cust.clone(), cell.tenant.0))
            })
            .collect()
    }
}

/// 单库（老）路径的默认租户：部署给定，不再「拿库里第一个 host 猜」。
pub(crate) const DEFAULT_TENANT_ENV: &str = "WIST_ATLAS_DEFAULT_TENANT";

fn default_tenant_of(file: &CellsFile) -> Option<String> {
    file.default_tenant
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn default_tenant() -> Option<String> {
    std::env::var(DEFAULT_TENANT_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
