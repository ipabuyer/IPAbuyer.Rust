//! 日志窗口管理（对齐 WinUI3 版独立 LogViewerWindow）。

use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::state::AppState;

/// 日志窗口标签（前端 main.tsx 按此分流渲染 LogWindow）。
pub const LOG_WINDOW_LABEL: &str = "log";

/// 打开日志窗口：已存在则显示并聚焦，不存在（用户已关闭）则重建。
/// 标题由后端 Fluent 按当前语言解析（window-title-log）。
/// async：窗口创建涉及异步初始化，官方建议在 async 命令中执行。
#[tauri::command]
pub async fn logs_show_window(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let title = crate::i18n::Lang::from_state(&state).message("window-title-log");
    match app.get_webview_window(LOG_WINDOW_LABEL) {
        Some(window) => {
            let _ = window.show();
            let _ = window.set_focus();
            Ok(())
        }
        None => WebviewWindowBuilder::new(&app, LOG_WINDOW_LABEL, WebviewUrl::App("index.html".into()))
            .title(title)
            .inner_size(760.0, 520.0)
            .min_inner_size(480.0, 320.0)
            // 日志内容恒为深色底（zinc-950），建窗即用深色避免白底闪现
            .background_color(crate::system_theme::window_background_color(true))
            .build()
            .map(|_| ())
            .map_err(|e| e.to_string()),
    }
}

/// 隐藏日志窗口（保留实例，再次打开免重建）。
#[tauri::command]
pub fn logs_hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window(LOG_WINDOW_LABEL) {
        let _ = window.hide();
    }
}
