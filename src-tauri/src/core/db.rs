//! 已购买数据库（SQLite，移植自主仓库 `PurchasedAppDb`）。
//!
//! schema 与主应用完全兼容：`PurchasedApp(Id, AppID, Account, Status)` +
//! `SyncState(Account, LastSuccessSyncUtc, LastAttemptSyncUtc)`；
//! 迁移语义对齐 C# `user_version`：0→1 状态归一化，1→2 owned 并入 purchased。
//! 账户与 AppID 沿用主应用归一化规则（trim + 小写）。数据库路径由宿主传入。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, params};

use crate::core::purchases::record_status;
use crate::core::purchases::sync_service::PurchaseStore;

/// 已购买记录的唯一规范状态。
pub const STATUS_PURCHASED: &str = "purchased";

/// 数据库错误。
#[derive(Debug)]
pub enum DbError {
    Sqlite(rusqlite::Error),
    InvalidStatus,
    Json(String),
}

impl From<rusqlite::Error> for DbError {
    fn from(error: rusqlite::Error) -> Self {
        DbError::Sqlite(error)
    }
}

impl From<serde_json::Error> for DbError {
    fn from(error: serde_json::Error) -> Self {
        DbError::Json(error.to_string())
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Sqlite(error) => write!(f, "{error}"),
            DbError::InvalidStatus => write!(f, "invalid purchase record status"),
            DbError::Json(message) => write!(f, "{message}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, DbError>;

/// 已购买数据库。内部单连接，线程安全。
pub struct PurchasedAppsDb {
    connection: Mutex<Connection>,
}

impl PurchasedAppsDb {
    /// 打开（必要时创建）数据库并执行 schema 初始化与迁移。
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS PurchasedApp (
                    Id INTEGER PRIMARY KEY AUTOINCREMENT,
                    AppID TEXT NOT NULL,
                    Account TEXT NOT NULL,
                    Status TEXT NOT NULL,
                    UNIQUE(AppID, Account)
                );
                CREATE INDEX IF NOT EXISTS idx_appid_account
                    ON PurchasedApp(AppID, Account);
                CREATE TABLE IF NOT EXISTS SyncState (
                    Account TEXT PRIMARY KEY,
                    LastSuccessSyncUtc TEXT NOT NULL,
                    LastAttemptSyncUtc TEXT NOT NULL
                );",
        )?;
        migrate_schema(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// 保存已购买应用（upsert；兼容历史大小写/空白差异，先按归一键更新）。
    pub fn save_purchased_app(
        &self,
        app_id: &str,
        account: &str,
        status: Option<&str>,
    ) -> Result<()> {
        let status = status.unwrap_or(record_status::PURCHASED);
        let Some(normalized_status) = record_status::try_normalize(Some(status)) else {
            return Err(DbError::InvalidStatus);
        };
        if app_id.trim().is_empty() || account.trim().is_empty() {
            return Ok(());
        }

        let (app_id, account) = (normalize(app_id), normalize(account));
        let connection = self.connection.lock().expect("db lock");
        let affected = connection.execute(
            "UPDATE PurchasedApp SET Status = $status
             WHERE LOWER(TRIM(AppID)) = $appid AND LOWER(TRIM(Account)) = $account",
            params![normalized_status, app_id, account],
        )?;
        if affected == 0 {
            connection.execute(
                "INSERT INTO PurchasedApp (AppID, Account, Status)
                 VALUES ($appid, $account, $status)
                 ON CONFLICT(AppID, Account) DO UPDATE SET Status = $status",
                params![app_id, account, normalized_status],
            )?;
        }
        Ok(())
    }

    /// 获取指定账户的全部已购买应用（按首次出现顺序去重，后写覆盖）。
    pub fn get_purchased_apps(&self, account: &str) -> Result<Vec<(String, String)>> {
        let mut list: Vec<(String, String)> = Vec::new();
        if account.trim().is_empty() {
            return Ok(list);
        }

        let connection = self.connection.lock().expect("db lock");
        let mut statement = connection.prepare(
            "SELECT AppID, Status FROM PurchasedApp
             WHERE LOWER(TRIM(Account)) = $account ORDER BY Id",
        )?;
        let rows = statement.query_map(params![normalize(account)], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        // 大小写不敏感去重：后写覆盖（对齐 C# 字典语义），保留首次出现顺序。
        let mut index_by_key: HashMap<String, usize> = HashMap::new();
        for row in rows {
            let (app_id, status) = row?;
            let app_id = normalize(&app_id);
            if app_id.is_empty() {
                continue;
            }
            let status = if status.trim().is_empty() {
                record_status::PURCHASED.to_string()
            } else {
                status
            };
            match index_by_key.get(&app_id) {
                Some(&index) => list[index].1 = status,
                None => {
                    index_by_key.insert(app_id.clone(), list.len());
                    list.push((app_id, status));
                }
            }
        }
        Ok(list)
    }

    /// 查询单个 App 状态；未记录返回 `None`。
    pub fn get_app_status(&self, app_id: &str, account: &str) -> Result<Option<String>> {
        if app_id.trim().is_empty() || account.trim().is_empty() {
            return Ok(None);
        }

        let connection = self.connection.lock().expect("db lock");
        let mut statement = connection.prepare(
            "SELECT Status FROM PurchasedApp
             WHERE LOWER(TRIM(AppID)) = $appid AND LOWER(TRIM(Account)) = $account
             ORDER BY Id DESC LIMIT 1",
        )?;
        let mut rows = statement.query(params![normalize(app_id), normalize(account)])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        }
        Ok(None)
    }

    /// 删除指定 App 的记录。
    pub fn remove_purchased_app(&self, app_id: &str, account: &str) -> Result<()> {
        if app_id.trim().is_empty() || account.trim().is_empty() {
            return Ok(());
        }

        let connection = self.connection.lock().expect("db lock");
        connection.execute(
            "DELETE FROM PurchasedApp
             WHERE LOWER(TRIM(AppID)) = $appid AND LOWER(TRIM(Account)) = $account",
            params![normalize(app_id), normalize(account)],
        )?;
        Ok(())
    }

    /// 清除记录；`account` 为空时清除全部。
    pub fn clear_purchased_apps(&self, account: Option<&str>) -> Result<()> {
        let connection = self.connection.lock().expect("db lock");
        match account {
            None => connection.execute("DELETE FROM PurchasedApp", [])?,
            Some(account) => connection.execute(
                "DELETE FROM PurchasedApp WHERE LOWER(TRIM(Account)) = $account",
                params![normalize(account)],
            )?,
        };
        Ok(())
    }

    /// 记录总数；`account` 为空时统计全部。
    pub fn get_total_count(&self, account: Option<&str>) -> Result<i64> {
        let connection = self.connection.lock().expect("db lock");
        let count = match account {
            None => {
                connection.query_row("SELECT COUNT(*) FROM PurchasedApp", [], |row| row.get(0))?
            }
            Some(account) => connection.query_row(
                "SELECT COUNT(*) FROM PurchasedApp WHERE LOWER(TRIM(Account)) = $account",
                params![normalize(account)],
                |row| row.get(0),
            )?,
        };
        Ok(count)
    }

    /// 批量标记为已购买（单事务 upsert），返回写入条数。
    pub fn bulk_mark_purchased(&self, bundle_ids: &[String], account: &str) -> Result<i64> {
        if bundle_ids.is_empty() || account.trim().is_empty() {
            return Ok(0);
        }

        let account = normalize(account);
        let connection = self.connection.lock().expect("db lock");
        let mut written = 0_usize;
        let transaction = connection.unchecked_transaction()?;
        for bundle_id in bundle_ids {
            if bundle_id.trim().is_empty() {
                continue;
            }
            written += transaction.execute(
                "INSERT INTO PurchasedApp (AppID, Account, Status)
                 VALUES ($appid, $account, $status)
                 ON CONFLICT(AppID, Account) DO UPDATE SET Status = $status",
                params![normalize(bundle_id), account, STATUS_PURCHASED],
            )?;
        }
        transaction.commit()?;
        Ok(written as i64)
    }

    /// 上次成功同步时间（RFC 3339 原文）；从未同步返回 `None`。
    pub fn get_last_successful_sync_utc(&self, account: &str) -> Result<Option<String>> {
        if account.trim().is_empty() {
            return Ok(None);
        }

        let connection = self.connection.lock().expect("db lock");
        let mut statement = connection
            .prepare("SELECT LastSuccessSyncUtc FROM SyncState WHERE Account = $account")?;
        let mut rows = statement.query(params![normalize(account)])?;
        if let Some(row) = rows.next()? {
            let text: String = row.get(0)?;
            if text.is_empty() {
                return Ok(None);
            }
            return Ok(Some(text));
        }
        Ok(None)
    }

    /// 记录一次同步尝试；成功时同时更新成功时间，失败时保留原值。
    pub fn record_sync_attempt(&self, account: &str, succeeded: bool) -> Result<()> {
        if account.trim().is_empty() {
            return Ok(());
        }

        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let connection = self.connection.lock().expect("db lock");
        connection.execute(
            "INSERT INTO SyncState (Account, LastSuccessSyncUtc, LastAttemptSyncUtc)
             VALUES ($account, CASE WHEN $succeeded THEN $now ELSE '' END, $now)
             ON CONFLICT(Account) DO UPDATE SET
                 LastSuccessSyncUtc = CASE WHEN $succeeded THEN $now ELSE SyncState.LastSuccessSyncUtc END,
                 LastAttemptSyncUtc = $now",
            params![normalize(account), succeeded, now],
        )?;
        Ok(())
    }
}

impl PurchaseStore for PurchasedAppsDb {
    fn bulk_mark_purchased(&mut self, bundle_ids: &[String], account: &str) {
        // 同步路径对持久化错误尽力而为：错误已由调用方日志记录。
        let _ = PurchasedAppsDb::bulk_mark_purchased(self, bundle_ids, account);
    }

    fn record_sync_attempt(&mut self, account: &str, succeeded: bool) {
        let _ = PurchasedAppsDb::record_sync_attempt(self, account, succeeded);
    }
}

/// schema 迁移（对齐 C# `MigrateSchema`）：0→1 状态归一化，1→2 owned 并入 purchased。
fn migrate_schema(connection: &Connection) -> Result<()> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version >= 2 {
        return Ok(());
    }

    let transaction = connection.unchecked_transaction()?;
    if version < 1 {
        let records: Vec<(i64, Option<String>)> = {
            let mut statement = transaction.prepare("SELECT Id, Status FROM PurchasedApp")?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<std::result::Result<Vec<_>, rusqlite::Error>>()?
        };

        for (id, status) in records {
            match record_status::try_normalize(status.as_deref()) {
                Some(normalized) => {
                    transaction.execute(
                        "UPDATE PurchasedApp SET Status = $status WHERE Id = $id",
                        params![normalized, id],
                    )?;
                }
                None => {
                    transaction.execute("DELETE FROM PurchasedApp WHERE Id = $id", params![id])?;
                }
            }
        }
    }

    if version < 2 {
        transaction.execute(
            "UPDATE PurchasedApp SET Status = $status WHERE Status <> $status",
            params![STATUS_PURCHASED],
        )?;
    }

    transaction.execute_batch("PRAGMA user_version = 2")?;
    transaction.commit()?;
    Ok(())
}

fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_db_path(tag: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir();
        directory.join(format!(
            "ipabuyer_core_db_{tag}_{}_{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    #[test]
    fn open_fresh_database_creates_schema_at_version_2() {
        let path = unique_db_path("fresh");
        let db = PurchasedAppsDb::open(&path).unwrap();

        assert_eq!(db.get_total_count(None).unwrap(), 0);
        let connection = Connection::open(&path).unwrap();
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn migration_merges_owned_into_purchased_and_normalizes_legacy_rows() {
        // 构造 legacy 数据库：version 0、混合状态（含无法识别的脏数据）。
        let path = unique_db_path("migrate");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE PurchasedApp (
                        Id INTEGER PRIMARY KEY AUTOINCREMENT,
                        AppID TEXT NOT NULL,
                        Account TEXT NOT NULL,
                        Status TEXT NOT NULL,
                        UNIQUE(AppID, Account)
                    );
                    INSERT INTO PurchasedApp (AppID, Account, Status) VALUES
                        ('com.purchased', 'user', 'purchased'),
                        ('com.owned', 'user', 'owned'),
                        ('com.localized', 'user', '已拥有'),
                        ('com.dirty', 'user', 'unknown-junk');
                    "#,
                )
                .unwrap();
        }

        let db = PurchasedAppsDb::open(&path).unwrap();

        assert_eq!(db.get_total_count(Some("user")).unwrap(), 3);
        assert_eq!(
            db.get_app_status("com.purchased", "user")
                .unwrap()
                .as_deref(),
            Some("purchased")
        );
        assert_eq!(
            db.get_app_status("com.owned", "user").unwrap().as_deref(),
            Some("purchased")
        );
        assert_eq!(
            db.get_app_status("com.localized", "user")
                .unwrap()
                .as_deref(),
            Some("purchased")
        );
        assert_eq!(db.get_app_status("com.dirty", "user").unwrap(), None);

        let connection = Connection::open(&path).unwrap();
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_get_remove_round_trip_with_normalization() {
        let path = unique_db_path("crud");
        let db = PurchasedAppsDb::open(&path).unwrap();

        db.save_purchased_app("  COM.App ", " User@Example.com ", None)
            .unwrap();
        assert_eq!(
            db.get_app_status("com.app", "user@example.com")
                .unwrap()
                .as_deref(),
            Some("purchased")
        );

        // 大小写不敏感更新 + 后写覆盖。
        db.save_purchased_app("com.app", "USER@example.com", Some("owned"))
            .unwrap();
        let apps = db.get_purchased_apps("user@example.com").unwrap();
        assert_eq!(apps, vec![("com.app".to_string(), "purchased".to_string())]);
        assert_eq!(db.get_total_count(None).unwrap(), 1);

        db.remove_purchased_app("com.app", "user@example.com")
            .unwrap();
        assert_eq!(db.get_total_count(None).unwrap(), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_rejects_invalid_status() {
        let path = unique_db_path("invalid");
        let db = PurchasedAppsDb::open(&path).unwrap();

        assert!(matches!(
            db.save_purchased_app("com.app", "user", Some("junk")),
            Err(DbError::InvalidStatus)
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn clear_supports_account_and_global_scope() {
        let path = unique_db_path("clear");
        let db = PurchasedAppsDb::open(&path).unwrap();

        db.save_purchased_app("com.a", "user1", None).unwrap();
        db.save_purchased_app("com.b", "user2", None).unwrap();

        db.clear_purchased_apps(Some("user1")).unwrap();
        assert_eq!(db.get_total_count(None).unwrap(), 1);

        db.clear_purchased_apps(None).unwrap();
        assert_eq!(db.get_total_count(None).unwrap(), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn bulk_mark_upserts_in_single_pass() {
        let path = unique_db_path("bulk");
        let db = PurchasedAppsDb::open(&path).unwrap();
        db.save_purchased_app("com.existing", "user", Some("owned"))
            .unwrap();

        let bundle_ids: Vec<String> = ["com.existing", "com.new", "  ", ""]
            .iter()
            .map(|id| id.to_string())
            .collect();
        let written = db.bulk_mark_purchased(&bundle_ids, "user").unwrap();

        assert_eq!(written, 2);
        assert_eq!(db.get_total_count(Some("user")).unwrap(), 2);
        assert_eq!(
            db.get_app_status("com.existing", "user")
                .unwrap()
                .as_deref(),
            Some("purchased")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sync_state_records_success_and_preserves_it_on_failure() {
        let path = unique_db_path("sync");
        let db = PurchasedAppsDb::open(&path).unwrap();

        assert!(db.get_last_successful_sync_utc("user").unwrap().is_none());

        db.record_sync_attempt("user", true).unwrap();
        let first = db.get_last_successful_sync_utc("user").unwrap().unwrap();
        assert!(!first.is_empty());
        assert!(chrono::DateTime::parse_from_rfc3339(&first).is_ok());

        db.record_sync_attempt("user", false).unwrap();
        assert_eq!(
            db.get_last_successful_sync_utc("user").unwrap().unwrap(),
            first
        );

        let connection = Connection::open(&path).unwrap();
        let attempt: String = connection
            .query_row(
                "SELECT LastAttemptSyncUtc FROM SyncState WHERE Account = 'user'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!attempt.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn purchase_store_impl_writes_through() {
        let path = unique_db_path("store");
        let mut db = PurchasedAppsDb::open(&path).unwrap();

        let bundle_ids = vec!["com.store".to_string()];
        PurchaseStore::bulk_mark_purchased(&mut db, &bundle_ids, "user");
        PurchaseStore::record_sync_attempt(&mut db, "user", true);

        assert_eq!(db.get_total_count(Some("user")).unwrap(), 1);
        assert!(db.get_last_successful_sync_utc("user").unwrap().is_some());
        let _ = std::fs::remove_file(&path);
    }
}
