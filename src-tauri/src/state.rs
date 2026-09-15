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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            country_code: crate::core::appcatalog::catalog_service::DEFAULT_COUNTRY_CODE.into(),
            download_directory: None,
            display_language: DISPLAY_LANGUAGE_AUTO.into(),
            detailed_ipatool_log: false,
            passphrase_rotation_enabled: false,
            ipatool_flavor: IPATOOL_FLAVOR_MAIN.into(),
            custom_ipatool_path: None,
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
        let config_path = data_dir.join("settings.json");
        let config = fs::read_to_string(&config_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Config>(&text).ok())
            .unwrap_or_default();

        let db_path = data_dir.join("PurchasedAppDb.db");
        let db = PurchasedAppsDb::open(&db_path)
            .map_err(|e| format!("打开已购数据库失败: {e}"))?;

        Ok(Self {
            config: Mutex::new(config),
            session: Mutex::new(Session::default()),
            db: Mutex::new(Some(db)),
            queue: QueueState::default(),
            sync: SyncState::default(),
            log_buffer: crate::commands::LogBuffer::new(),
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
