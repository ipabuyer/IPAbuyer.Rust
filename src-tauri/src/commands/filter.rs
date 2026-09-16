//! 主页筛选：独立筛选窗口（label `filter`，仿日志窗口）与跨窗口筛选状态。
//!
//! 筛选选择（平台/开发者）与开发者选项保存在后端，作为筛选窗口与主窗口
//! （两个 WebView，状态不共享）之间的唯一事实来源；任何变更经
//! `filter-changed` 事件推送给两个窗口。开发者选项由 `catalog_search`
//! 按最新搜索结果刷新，已选开发者不在新结果中时自动回退"所有开发者"。

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::state::AppState;

/// 筛选窗口标签（前端 main.tsx 按此分流渲染 FilterWindow）。
pub const FILTER_WINDOW_LABEL: &str = "filter";

/// 允许的平台筛选值（"all" 为 UI 概念，不属于 AppPlatform）。
const PLATFORM_FILTERS: [&str; 4] = ["all", "ios", "ipad", "macos"];

/// 筛选选择（platform/developer 为规范 token，显示文案由前端 i18next 渲染）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterSelection {
    pub platform: String,
    pub developer: String,
    pub developers: Vec<String>,
}

impl Default for FilterSelection {
    fn default() -> Self {
        Self {
            platform: "all".into(),
            developer: "all".into(),
            developers: Vec::new(),
        }
    }
}

fn emit_filter(app: &AppHandle, selection: &FilterSelection) {
    let _ = app.emit("filter-changed", selection);
}

/// 获取当前筛选选择。
#[tauri::command]
pub fn filter_get(state: State<'_, AppState>) -> FilterSelection {
    state.filter.lock().unwrap().clone()
}

/// 更新筛选选择（platform/developer 传 None 表示不变），并推送事件。
#[tauri::command]
pub fn filter_set(
    app: AppHandle,
    state: State<'_, AppState>,
    platform: Option<String>,
    developer: Option<String>,
) -> Result<(), String> {
    let mut filter = state.filter.lock().unwrap();
    if let Some(platform) = platform.as_deref() {
        let platform = platform.trim();
        if !PLATFORM_FILTERS.contains(&platform) {
            return Err(format!(
                "{}",
                crate::i18n::Lang::from_state(&state)
                    .message_with("error-invalid-platform-filter", &[("value", platform)]),
            ));
        }
        filter.platform = platform.into();
    }
    if let Some(developer) = developer.as_deref() {
        let developer = developer.trim();
        filter.developer = if developer.is_empty() {
            "all".into()
        } else {
            developer.into()
        };
    }
    let selection = filter.clone();
    drop(filter);
    emit_filter(&app, &selection);
    Ok(())
}

/// 搜索完成后刷新开发者选项；已选开发者不在新结果中时回退"所有开发者"。
pub fn refresh_developer_options(app: &AppHandle, state: &AppState, developers: Vec<String>) {
    let mut filter = state.filter.lock().unwrap();
    apply_developer_options(&mut filter, developers);
    let selection = filter.clone();
    drop(filter);
    emit_filter(app, &selection);
}

/// 刷新开发者选项的纯逻辑：选项替换 + 失效开发者回退（大小写不敏感比较）。
fn apply_developer_options(selection: &mut FilterSelection, developers: Vec<String>) {
    selection.developers = developers;
    let stale = selection.developer != "all"
        && !selection
            .developers
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&selection.developer));
    if stale {
        selection.developer = "all".into();
    }
}

/// 打开筛选窗口：已存在则显示并聚焦，不存在（用户已关闭）则重建。
/// 标题由后端 Fluent 按当前语言解析（window-title-filter）。
/// async：窗口创建涉及异步初始化，官方建议在 async 命令中执行。
#[tauri::command]
pub async fn filter_show_window(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let title = crate::i18n::Lang::from_state(&state).message("window-title-filter");
    match app.get_webview_window(FILTER_WINDOW_LABEL) {
        Some(window) => {
            let _ = window.show();
            let _ = window.set_focus();
            Ok(())
        }
        None => WebviewWindowBuilder::new(&app, FILTER_WINDOW_LABEL, WebviewUrl::App("index.html".into()))
            .title(title)
            .inner_size(340.0, 280.0)
            .resizable(false)
            .build()
            .map(|_| ())
            .map_err(|e| e.to_string()),
    }
}

/// 隐藏筛选窗口（保留实例，再次打开免重建）。
#[tauri::command]
pub fn filter_hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window(FILTER_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection_is_all() {
        let selection = FilterSelection::default();
        assert_eq!(selection.platform, "all");
        assert_eq!(selection.developer, "all");
        assert!(selection.developers.is_empty());
    }

    #[test]
    fn apply_developer_options_resets_stale_developer() {
        let mut selection = FilterSelection {
            developer: "  TENCENT ".into(),
            developers: vec!["Tencent".into(), "NetEase".into()],
            ..FilterSelection::default()
        };

        apply_developer_options(&mut selection, vec!["NetEase".into(), "Apple".into()]);

        assert_eq!(selection.developers, vec!["NetEase", "Apple"]);
        // 已选开发者不在新结果中 → 回退全部
        assert_eq!(selection.developer, "all");
    }

    #[test]
    fn apply_developer_options_keeps_case_insensitive_match() {
        let mut selection = FilterSelection {
            developer: "tencent".into(),
            ..FilterSelection::default()
        };

        apply_developer_options(&mut selection, vec!["Tencent".into()]);

        assert_eq!(selection.developer, "tencent");
        assert_eq!(selection.developers, vec!["Tencent"]);
    }
}
