//! 应用级共享状态：配置存储、会话、已购数据库、密钥（passphrase）管理。
//!
//! 对应原 C# 门面：ConfigurationStore / SessionState / PurchasedAppDb /
//! PassphraseStore。设置持久化为 app data 目录下的 settings.json；
//! 密钥存 Windows 凭据管理器（keyring）。

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::db::PurchasedAppsDb;

pub const PASSPHRASE_SERVICE: &str = "IPAbuyer.ipatool.passphrase";
pub const PASSPHRASE_USER: &str = "__default__";
pub const DISPLAY_LANGUAGE_AUTO: &str = "auto";
pub const IPATOOL_FLAVOR_MAIN: &str = "main";
pub const IPATOOL_FLAVOR_CUSTOM: &str = "custom";

/// 应用设置（settings.json，serde(default) 保证新增字段向后兼容）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub country_code: String,
    pub download_directory: Option<String>,
    pub display_language: String,
    pub detailed_ipatool_log: bool,
    pub passphrase_rotation_enabled: bool,
    pub ipatool_flavor: String,
    pub custom_ipatool_path: Option<String>,
    /// 旧版 WinUI3 数据库是否已导入（避免重复导入覆盖新数据）
    pub legacy_db_imported: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            country_code: crate::core::appcatalog::catalog_service::DEFAULT_COUNTRY_CODE.into(),
            download_directory: None,
            display_language: DISPLAY_LANGUAGE_AUTO.into(),
            detailed_ipatool_log: false,
            // 产品决策：密钥轮换默认开启（登出自动生成新密钥，见 DEVELOPMENT.md 15）
            passphrase_rotation_enabled: true,
            ipatool_flavor: IPATOOL_FLAVOR_MAIN.into(),
            custom_ipatool_path: None,
            legacy_db_imported: false,
        }
    }
}

impl Config {
    /// 默认下载目录：~/Downloads（不存在则创建），对齐 C# GetDefaultDownloadDirectory。
    pub fn default_download_directory() -> PathBuf {
        let dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Downloads");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn download_directory(&self) -> PathBuf {
        self.download_directory
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(Self::default_download_directory)
    }
}

/// 进程内会话状态（对齐 C# SessionState）。
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub account: Option<String>,
    pub logged_in: bool,
    pub is_mock: bool,
}

impl Session {
    pub fn set_login(&mut self, account: String, is_mock: bool) {
        self.account = Some(account);
        self.logged_in = true;
        self.is_mock = is_mock;
    }

    pub fn reset(&mut self) {
        self.account = None;
        self.logged_in = false;
        self.is_mock = false;
    }
}

/// Tauri managed state。
pub struct AppState {
    pub config: Mutex<Config>,
    pub session: Mutex<Session>,
    pub db: Mutex<Option<PurchasedAppsDb>>,
    pub queue: QueueState,
    pub sync: SyncState,
    pub log_buffer: crate::commands::LogBuffer,
    pub filter: Mutex<crate::commands::filter::FilterSelection>,
    config_path: PathBuf,
    db_path: PathBuf,
}

/// 下载队列共享状态：队列服务与队列级取消标志。
pub struct QueueState {
    pub queue: std::sync::Arc<crate::core::downloads::queue::DownloadQueueService>,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            queue: std::sync::Arc::new(crate::core::downloads::queue::DownloadQueueService::new()),
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

/// 已购同步共享状态。
pub struct SyncState {
    pub service: crate::core::purchases::sync_service::PurchaseSyncService,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            service: crate::core::purchases::sync_service::PurchaseSyncService::new(),
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

impl AppState {
    /// 在 Tauri setup 阶段调用：初始化数据目录、加载设置、打开数据库。
    pub fn new(data_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
        migrate_legacy_passphrase();
        let config_path = data_dir.join("settings.json");
        let config = fs::read_to_string(&config_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Config>(&text).ok())
            .unwrap_or_default();

        let db_path = data_dir.join("PurchasedAppDb.db");
        let db = PurchasedAppsDb::open(&db_path).map_err(|e| {
            let lang = crate::i18n::Lang::from_config(&config.display_language);
            lang.message_with("error-open-db-failed", &[("error", &e.to_string())])
        })?;

        Ok(Self {
            config: Mutex::new(config),
            session: Mutex::new(Session::default()),
            db: Mutex::new(Some(db)),
            queue: QueueState::default(),
            sync: SyncState::default(),
            log_buffer: crate::commands::LogBuffer::new(),
            filter: Mutex::new(crate::commands::filter::FilterSelection::default()),
            config_path,
            db_path: db_path.clone(),
        })
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    pub fn save_config(&self, config: &Config) -> Result<(), String> {
        let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, text).map_err(|e| e.to_string())
    }

    /// 修改设置并落盘。
    pub fn update_config(&self, update: impl FnOnce(&mut Config)) -> Result<Config, String> {
        let mut guard = self.config.lock().unwrap();
        update(&mut guard);
        self.save_config(&guard)?;
        Ok(guard.clone())
    }
}

/// 读取密钥：未存储时返回 None（调用方生成新密钥，登录成功后落库）。
pub fn get_passphrase() -> Option<String> {
    keyring::Entry::new(PASSPHRASE_SERVICE, PASSPHRASE_USER)
        .and_then(|e| e.get_password())
        .ok()
        .filter(|p| !p.trim().is_empty())
}

/// 保存密钥。
pub fn save_passphrase(passphrase: &str) -> Result<(), String> {
    keyring::Entry::new(PASSPHRASE_SERVICE, PASSPHRASE_USER)
        .and_then(|e| e.set_password(passphrase))
        .map_err(|e| e.to_string())
}

/// 生成新密钥（32 位十六进制，对齐 C# Guid.NewGuid("N")）。
pub fn generate_passphrase() -> String {
    Uuid::new_v4().simple().to_string()
}

/// 旧 WinUI3 版把密钥存于 WinRT PasswordVault（同名同用户）。本应用改用
/// 凭据管理器，首次启动时若新存储尚无密钥则从旧存储迁移，商店升级用户
/// 无需重新登录。任何失败都静默跳过（保持未登录状态下的正常流程）。
#[cfg(windows)]
fn migrate_legacy_passphrase() {
    if get_passphrase().is_some() {
        return;
    }
    if let Some(passphrase) = read_legacy_password_vault() {
        let _ = save_passphrase(&passphrase);
    }
}

#[cfg(windows)]
fn read_legacy_password_vault() -> Option<String> {
    use windows::Security::Credentials::PasswordVault;

    let vault = PasswordVault::new().ok()?;
    let credential = vault
        .Retrieve(
            &windows::core::HSTRING::from(PASSPHRASE_SERVICE),
            &windows::core::HSTRING::from(PASSPHRASE_USER),
        )
        .ok()?;
        let password = credential.Password().ok()?;
        let password = password.to_string_lossy();
        (!password.is_empty()).then_some(password)
}

#[cfg(not(windows))]
fn migrate_legacy_passphrase() {}

/// settings.json 路径（与 Tauri app_data_dir 解析一致：%APPDATA%/{identifier}）。
pub fn config_file_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(crate::IDENTIFIER)
        .join("settings.json")
}

/// 读取显示语言偏好（auto 或无效值返回 None，由前端按系统语言推断）。
pub fn read_display_language(path: &PathBuf) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let config: Config = serde_json::from_str(&text).ok()?;
    let lang = config.display_language;
    (lang == "zh-Hans" || lang == "en-US").then_some(lang)
}

/// 解析登录用密钥：显式传入优先，其次读取已存密钥，均无则生成新密钥。
/// 返回 (密钥, 是否新生成——由调用方在登录成功后落库)。
pub fn resolve_passphrase(explicit: Option<&str>) -> (String, bool) {
    if let Some(p) = explicit {
        let p = p.trim();
        if !p.is_empty() {
            return (p.to_string(), false);
        }
    }
    match get_passphrase() {
        Some(p) => (p, false),
        None => (generate_passphrase(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 密钥轮换默认开启是产品决策（2026-09），防止无意识回退。
    #[test]
    fn passphrase_rotation_defaults_to_enabled() {
        assert!(Config::default().passphrase_rotation_enabled);
    }

    /// settings.json 落盘为 snake_case，缺字段回落默认值（含密钥轮换 true）。
    #[test]
    fn config_serde_snake_case_round_trip_with_defaults() {
        let json = r#"{
            "country_code": "us",
            "display_language": "en-US",
            "legacy_db_imported": true
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.country_code, "us");
        assert_eq!(config.display_language, "en-US");
        assert!(config.legacy_db_imported);
        assert!(config.passphrase_rotation_enabled);
        assert!(config.download_directory.is_none());
        assert_eq!(config.ipatool_flavor, "main");

        let serialized = serde_json::to_string(&Config::default()).unwrap();
        assert!(serialized.contains("\"country_code\""));
        assert!(!serialized.contains("countryCode"));
    }

    #[test]
    fn resolve_passphrase_prefers_explicit_input_without_touching_store() {
        let (passphrase, generated) = resolve_passphrase(Some("  abc123  "));
        assert_eq!(passphrase, "abc123");
        assert!(!generated);
    }

    #[test]
    fn read_display_language_accepts_only_known_values() {
        let dir = std::env::temp_dir().join("ipabuyer-state-test");
        fs::create_dir_all(&dir).unwrap();

        let write = |name: &str, text: &str| {
            let path = dir.join(name);
            fs::write(&path, text).unwrap();
            path
        };
        let zh = write("zh.json", r#"{"display_language": "zh-Hans"}"#);
        let en = write("en.json", r#"{"display_language": "en-US"}"#);
        let auto = write("auto.json", r#"{"display_language": "auto"}"#);
        let broken = write("broken.json", "not json");
        let path = zh.clone();

        assert_eq!(read_display_language(&zh), Some("zh-Hans".into()));
        assert_eq!(read_display_language(&en), Some("en-US".into()));
        assert_eq!(read_display_language(&auto), None);
        assert_eq!(read_display_language(&broken), None);
        assert_eq!(read_display_language(&dir.join("missing.json")), None);

        drop(path);
        let _ = fs::remove_dir_all(&dir);
    }
}
