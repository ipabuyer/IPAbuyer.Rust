//! 设置命令（对齐 C# ConfigurationStore / ApplicationSettings）。

use serde::Serialize;
use tauri::{Emitter, State};

use crate::state::{AppState, Config, DISPLAY_LANGUAGE_AUTO};
use crate::storefront;

/// 返回给前端的设置 DTO（camelCase）。settings.json 落盘仍为 snake_case，
/// 序列化格式变化只发生在命令边界。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDto {
    pub country_code: String,
    pub download_directory: Option<String>,
    pub display_language: String,
    pub detailed_ipatool_log: bool,
    pub passphrase_rotation_enabled: bool,
    pub ipatool_flavor: String,
    pub custom_ipatool_path: Option<String>,
    pub legacy_db_imported: bool,
}

impl From<Config> for ConfigDto {
    fn from(c: Config) -> Self {
        ConfigDto {
            country_code: c.country_code,
            download_directory: c.download_directory,
            display_language: c.display_language,
            detailed_ipatool_log: c.detailed_ipatool_log,
            passphrase_rotation_enabled: c.passphrase_rotation_enabled,
            ipatool_flavor: c.ipatool_flavor,
            custom_ipatool_path: c.custom_ipatool_path,
            legacy_db_imported: c.legacy_db_imported,
        }
    }
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> ConfigDto {
    ConfigDto::from(state.config.lock().unwrap().clone())
}

#[tauri::command]
pub fn settings_default_download_directory() -> String {
    Config::default_download_directory()
        .to_string_lossy()
        .into_owned()
}

fn set_country_code_inner(state: &AppState, code: String) -> Result<ConfigDto, String> {
    let normalized = code.trim().to_lowercase();
    if normalized.is_empty() || !storefront::contains(&normalized) {
        return Err(format!("无效的国家/地区代码: {code}"));
    }
    state.update_config(|c| c.country_code = normalized).map(ConfigDto::from)
}

#[tauri::command]
pub fn settings_set_country_code(state: State<'_, AppState>, code: String) -> Result<ConfigDto, String> {
    set_country_code_inner(&state, code)
}

fn set_download_directory_inner(state: &AppState, path: String) -> Result<ConfigDto, String> {
    let trimmed = path.trim().to_string();
    if trimmed.is_empty() {
        return Err("下载目录不能为空".into());
    }
    if !std::path::Path::new(&trimmed).is_dir() {
        return Err(format!("目录不存在: {trimmed}"));
    }
    state.update_config(|c| c.download_directory = Some(trimmed)).map(ConfigDto::from)
}

#[tauri::command]
pub fn settings_set_download_directory(
    state: State<'_, AppState>,
    path: String,
) -> Result<ConfigDto, String> {
    set_download_directory_inner(&state, path)
}

#[tauri::command]
pub fn settings_reset_download_directory(state: State<'_, AppState>) -> Result<ConfigDto, String> {
    state.update_config(|c| c.download_directory = None).map(ConfigDto::from)
}

fn set_display_language_inner(state: &AppState, language: String) -> Result<ConfigDto, String> {
    let value = language.trim().to_string();
    if value != DISPLAY_LANGUAGE_AUTO && value != "zh-Hans" && value != "en-US" {
        return Err(format!("无效的显示语言: {language}"));
    }
    state.update_config(|c| c.display_language = value).map(ConfigDto::from)
}

/// 显示语言：auto / zh-Hans / en-US。
/// 保存后广播 `language-changed`，所有窗口（主窗口/日志/筛选）同步切换。
#[tauri::command]
pub fn settings_set_display_language(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    language: String,
) -> Result<ConfigDto, String> {
    let config = set_display_language_inner(&state, language)?;
    let _ = app.emit("language-changed", config.display_language.clone());
    Ok(config)
}

#[tauri::command]
pub fn settings_set_detailed_log(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<ConfigDto, String> {
    state.update_config(|c| c.detailed_ipatool_log = enabled).map(ConfigDto::from)
}

#[tauri::command]
pub fn settings_set_passphrase_rotation(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<ConfigDto, String> {
    state.update_config(|c| c.passphrase_rotation_enabled = enabled).map(ConfigDto::from)
}

/// 页面加载时预填密钥输入框：优先已存密钥，否则生成新密钥（不落库，
/// 登录成功后才保存，对齐 C# PassphraseStore.Get 的行为）。
#[tauri::command]
pub fn settings_get_passphrase() -> String {
    crate::state::get_passphrase().unwrap_or_else(crate::state::generate_passphrase)
}

/// Apple storefront 目录（国家/地区代码 + 英文名），供选择对话框。
#[tauri::command]
pub fn settings_list_storefronts() -> Vec<(String, String)> {
    crate::storefront::STOREFRONTS
        .iter()
        .map(|(code, name)| (code.to_string(), name.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 命令边界出参为 camelCase，字段逐一对应 Config（前端 AppConfig 镜像）。
    #[test]
    fn config_dto_maps_all_fields() {
        let config = Config {
            country_code: "cn".into(),
            download_directory: Some("D:/dl".into()),
            display_language: "zh-Hans".into(),
            detailed_ipatool_log: true,
            passphrase_rotation_enabled: true,
            ipatool_flavor: "custom".into(),
            custom_ipatool_path: Some("C:/tools/ipatool.exe".into()),
            legacy_db_imported: true,
        };
        let dto = ConfigDto::from(config);
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["countryCode"], "cn");
        assert_eq!(json["downloadDirectory"], "D:/dl");
        assert_eq!(json["displayLanguage"], "zh-Hans");
        assert_eq!(json["detailedIpatoolLog"], true);
        assert_eq!(json["passphraseRotationEnabled"], true);
        assert_eq!(json["ipatoolFlavor"], "custom");
        assert_eq!(json["customIpatoolPath"], "C:/tools/ipatool.exe");
        assert_eq!(json["legacyDbImported"], true);
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::state::AppState;

    fn temp_state(name: &str) -> AppState {
        let dir = std::env::temp_dir().join(format!("ipabuyer-settings-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        AppState::new(dir).expect("temp AppState")
    }

    #[test]
    fn set_country_code_rejects_unknown_and_accepts_valid() {
        let state = temp_state("cc");
        assert!(set_country_code_inner(&state, "XX".into()).is_err());
        assert!(set_country_code_inner(&state, "  ".into()).is_err());

        let dto = set_country_code_inner(&state, " JP ".into()).unwrap();
        assert_eq!(dto.country_code, "jp");
    }

    #[test]
    fn set_country_code_persists_settings_json() {
        let state = temp_state("cc-persist");
        set_country_code_inner(&state, "de".into()).unwrap();
        let on_disk = std::fs::read_to_string(state.db_path().parent().unwrap().join("settings.json"))
            .unwrap();
        assert!(on_disk.contains("\"country_code\": \"de\""));
    }

    #[test]
    fn set_download_directory_requires_existing_dir() {
        let state = temp_state("dl");
        assert!(set_download_directory_inner(&state, "".into()).is_err());
        assert!(set_download_directory_inner(&state, "Z:/missing/dir".into()).is_err());

        let dir = std::env::temp_dir().join(format!("ipabuyer-dl-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dto = set_download_directory_inner(&state, dir.to_string_lossy().into_owned()).unwrap();
        assert_eq!(dto.download_directory.as_deref(), Some(dir.to_string_lossy().as_ref()));
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn set_display_language_validates_values() {
        let state = temp_state("lang");
        assert!(set_display_language_inner(&state, "fr-FR".into()).is_err());
        assert!(set_display_language_inner(&state, "  ".into()).is_err());
        assert_eq!(
            set_display_language_inner(&state, " en-US ".into()).unwrap().display_language,
            "en-US"
        );
    }
}
