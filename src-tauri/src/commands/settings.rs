//! 设置命令（对齐 C# ConfigurationStore / ApplicationSettings）。

use tauri::State;

use crate::state::{AppState, Config, DISPLAY_LANGUAGE_AUTO};
use crate::storefront;

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
pub fn settings_default_download_directory() -> String {
    Config::default_download_directory()
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
pub fn settings_set_country_code(state: State<'_, AppState>, code: String) -> Result<Config, String> {
    let normalized = code.trim().to_lowercase();
    if normalized.is_empty() || !storefront::contains(&normalized) {
        return Err(format!("无效的国家/地区代码: {code}"));
    }
    state.update_config(|c| c.country_code = normalized)
}

#[tauri::command]
pub fn settings_set_download_directory(
    state: State<'_, AppState>,
    path: String,
) -> Result<Config, String> {
    let trimmed = path.trim().to_string();
    if trimmed.is_empty() {
        return Err("下载目录不能为空".into());
    }
    if !std::path::Path::new(&trimmed).is_dir() {
        return Err(format!("目录不存在: {trimmed}"));
    }
    state.update_config(|c| c.download_directory = Some(trimmed))
}

#[tauri::command]
pub fn settings_reset_download_directory(state: State<'_, AppState>) -> Result<Config, String> {
    state.update_config(|c| c.download_directory = None)
}

/// 显示语言：auto / zh-Hans / en-US。
#[tauri::command]
pub fn settings_set_display_language(
    state: State<'_, AppState>,
    language: String,
) -> Result<Config, String> {
    let value = language.trim().to_string();
    if value != DISPLAY_LANGUAGE_AUTO && value != "zh-Hans" && value != "en-US" {
        return Err(format!("无效的显示语言: {language}"));
    }
    state.update_config(|c| c.display_language = value)
}

#[tauri::command]
pub fn settings_set_detailed_log(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Config, String> {
    state.update_config(|c| c.detailed_ipatool_log = enabled)
}

#[tauri::command]
pub fn settings_set_passphrase_rotation(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Config, String> {
    state.update_config(|c| c.passphrase_rotation_enabled = enabled)
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
