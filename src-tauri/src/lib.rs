//! IPAbuyer Tauri 后端。
//!
//! 分层：`core`（业务核心，并入自 IPAbuyer.Core）→ `state`/`commands`/`events`
//! （复刻原 C# 门面职责：配置存储、会话、队列/同步编排、日志缓冲、事件推送）。

pub mod commands;
pub mod core;
pub mod events;
pub mod i18n;
pub mod resolver;
pub mod state;
pub mod storefront;
pub mod system_theme;

// M4: 同步与 ipatool 管理命令模块（commands 子模块）

use tauri::Manager;

/// 应用标识（须与 tauri.conf.json 的 identifier 保持一致）。
pub const IDENTIFIER: &str = "com.ipabuyer.app";

/// 构建 Tauri 应用并运行。
pub fn run() {
    // 显示语言与系统主题在 webview 脚本执行前注入，保证首帧即为偏好语言/主题
    let display_language = state::read_display_language(&state::config_file_path());
    let theme = system_theme::current_system_theme();
    let init_script = initialization_script(display_language.as_deref(), theme);

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .append_invoke_initialization_script(&init_script)
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| crate::i18n::Lang::system().message_with(
                    "error-resolve-data-dir-failed",
                    &[("error", &e.to_string())],
                ))?;
            let state = state::AppState::new(data_dir).map_err(std::io::Error::other)?;
            app.manage(state);
            events::start_polling(app.handle().clone());
            system_theme::start_theme_watcher(app.handle().clone());
            apply_startup_theme(app.handle());
            fit_main_window(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::settings_get,
            commands::settings::settings_default_download_directory,
            commands::settings::settings_set_country_code,
            commands::settings::settings_set_download_directory,
            commands::settings::settings_reset_download_directory,
            commands::settings::settings_set_display_language,
            commands::settings::settings_set_detailed_log,
            commands::settings::settings_set_passphrase_rotation,
            commands::settings::settings_get_passphrase,
            commands::settings::settings_list_storefronts,
            commands::catalog::catalog_search,
            commands::filter::filter_get,
            commands::filter::filter_set,
            commands::filter::filter_show_window,
            commands::filter::filter_hide_window,
            commands::purchases::purchase,
            commands::purchases::purchases_mark,
            commands::purchases::purchases_unmark,
            commands::queue::queue_add,
            commands::queue::queue_start,
            commands::queue::queue_status,
            commands::queue::queue_cancel_current,
            commands::queue::logs_clear,
            commands::queue::logs_snapshot,
            commands::logs::logs_show_window,
            commands::logs::logs_hide_window,
            commands::theme::system_theme,
            commands::sync::sync_start,
            commands::sync::sync_cancel,
            commands::sync::sync_status,
            commands::sync::sync_last_time,
            commands::ipatool::ipatool_info,
            commands::ipatool::ipatool_set_flavor,
            commands::ipatool::ipatool_set_custom_path,
            commands::ipatool::ipatool_delete_custom,
            commands::ipatool::ipatool_export,
            commands::ipatool::ipatool_clear_data,
            commands::ipatool::legacy_db_exists,
            commands::ipatool::legacy_db_import,
            commands::auth::auth_login,
            commands::auth::auth_verify_code,
            commands::auth::auth_logout,
            commands::auth::auth_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::initialization_script;

    #[test]
    fn initialization_script_exposes_language_and_theme() {
        assert_eq!(
            initialization_script(Some("zh-Hans"), "dark"),
            "window.__IPABUYER_LANG__ = \"zh-Hans\";\nwindow.__IPABUYER_THEME__ = \"dark\";"
        );
        assert_eq!(
            initialization_script(None, "light"),
            "window.__IPABUYER_LANG__ = undefined;\nwindow.__IPABUYER_THEME__ = \"light\";"
        );
    }
}

/// webview 初始化脚本：在任何页面脚本（含 index.html 内联脚本）执行前，
/// 暴露显示语言与系统主题，供首绘前应用（深色模式防白屏，见 index.html）。
fn initialization_script(display_language: Option<&str>, theme: &str) -> String {
    let lang = display_language
        .map(|l| format!("{l:?}"))
        .unwrap_or_else(|| "undefined".into());
    format!("window.__IPABUYER_LANG__ = {lang};\nwindow.__IPABUYER_THEME__ = {theme:?};")
}

/// 启动主题适配：深色模式下主窗口与 WebView 底色先用深色——页面内容
/// 首绘前 WebView 默认白底，深色模式会白屏闪眼；浅色即默认值无需处理。
fn apply_startup_theme(app: &tauri::AppHandle) {
    if system_theme::current_system_theme() != "dark" {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_background_color(Some(system_theme::window_background_color(true)));
        let _ = window.set_theme(Some(tauri::Theme::Dark));
    }
}

/// 主窗口启动适配：钳制在当前显示器工作区（去除任务栏）内。
///
/// 窗口几何在此时已被 window-state 插件恢复（或为 tauri.conf.json 默认值）：
/// 尺寸超限收缩、位置越界回位，用户已保存的几何尽量保留；无状态文件的
/// 首次启动才居中。内置 center() 以整块显示器为基准，任务栏在下方时仍会
/// 压入，故按 work_area 自行计算。
fn fit_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_maximized().unwrap_or(false) {
        return;
    }
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    let avail_w = work_area.size.width as f64 / scale;
    let avail_h = work_area.size.height as f64 / scale;

    // 尺寸：恢复值/默认值超出工作区才收缩
    let Ok(size) = window.outer_size() else {
        return;
    };
    let cur_w = size.width as f64 / scale;
    let cur_h = size.height as f64 / scale;
    let width = cur_w.min(avail_w);
    let height = cur_h.min(avail_h);
    if width != cur_w || height != cur_h {
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
    }

    // 位置：越界回位（负坐标、压任务栏、被副屏甩出等）
    let win_w = (width * scale).round() as i32;
    let win_h = (height * scale).round() as i32;
    let max_x = (work_area.size.width as i32 - win_w).max(0) + work_area.position.x;
    let max_y = (work_area.size.height as i32 - win_h).max(0) + work_area.position.y;
    let Ok(pos) = window.outer_position() else {
        return;
    };
    let x = pos.x.clamp(work_area.position.x, max_x);
    let y = pos.y.clamp(work_area.position.y, max_y);
    if x != pos.x || y != pos.y {
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        return;
    }

    // 位置在工作区内且无已保存状态（首次启动）：在工作区居中
    let first_run = app
        .path()
        .app_config_dir()
        .ok()
        .is_some_and(|dir| !dir.join(tauri_plugin_window_state::DEFAULT_FILENAME).exists());
    if first_run {
        let _ = window.set_position(tauri::PhysicalPosition::new(
            work_area.position.x
                + ((work_area.size.width as f64 - width * scale) / 2.0).round() as i32,
            work_area.position.y
                + ((work_area.size.height as f64 - height * scale) / 2.0).round() as i32,
        ));
    }
}
