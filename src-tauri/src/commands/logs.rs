//! 日志窗口管理（对齐 WinUI3 版独立 LogViewerWindow）。

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// 日志窗口标签（前端 main.tsx 按此分流渲染 LogWindow）。
pub const LOG_WINDOW_LABEL: &str = "log";

/// 打开日志窗口：已存在则显示并聚焦，不存在（用户已关闭）则重建。
/// 标题由前端按当前语言传入（原生标题栏无法使用前端 i18n 资源）。
/// async：窗口创建涉及异步初始化，官方建议在 async 命令中执行。
#[tauri::command]
pub async fn logs_show_window(app: AppHandle, title: Option<String>) -> Result<(), String> {
    match app.get_webview_window(LOG_WINDOW_LABEL) {
        Some(window) => {
            let _ = window.show();
            let _ = window.set_focus();
            Ok(())
        }
        None => WebviewWindowBuilder::new(&app, LOG_WINDOW_LABEL, WebviewUrl::App("index.html".into()))
            .title(title.unwrap_or_else(|| LOG_WINDOW_LABEL.into()))
            .inner_size(760.0, 520.0)
            .min_inner_size(480.0, 320.0)
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
