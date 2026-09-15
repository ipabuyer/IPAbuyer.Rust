//! IPAbuyer Tauri 后端。
//!
//! 分层：`core`（业务核心，并入自 IPAbuyer.Core）→ `state`/`commands`/`events`
//! （复刻原 C# 门面职责：配置存储、会话、队列/同步编排、日志缓冲、事件推送）。

pub mod commands;
pub mod core;
pub mod events;
pub mod resolver;
pub mod state;
pub mod storefront;

// M4: 同步与 ipatool 管理命令模块（commands 子模块）

use tauri::Manager;

/// 应用标识（须与 tauri.conf.json 的 identifier 保持一致）。
pub const IDENTIFIER: &str = "com.ipabuyer.app";

/// 构建 Tauri 应用并运行。
pub fn run() {
    // 显示语言在 webview 脚本执行前注入，保证首帧即为偏好语言
    let display_language = state::read_display_language(&state::config_file_path());
    let init_script = format!(
        "window.__IPABUYER_LANG__ = {};",
        display_language
            .map(|l| format!("{l:?}"))
            .unwrap_or_else(|| "undefined".into())
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .append_invoke_initialization_script(&init_script)
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("解析数据目录失败: {e}"))?;
            let state = state::AppState::new(data_dir).map_err(std::io::Error::other)?;
            app.manage(state);
            events::start_polling(app.handle().clone());
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
            commands::purchases::purchase,
            commands::purchases::purchases_mark,
            commands::purchases::purchases_unmark,
            commands::queue::queue_add,
            commands::queue::queue_start,
            commands::queue::queue_status,
            commands::queue::queue_cancel_current,
            commands::queue::logs_clear,
            commands::queue::logs_snapshot,
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

/// 主窗口初始尺寸/位置：钳制在当前显示器工作区（去除任务栏）内并居中。
///
/// tauri.conf.json 的固定尺寸在小屏或高 DPI 缩放下会超出可用区域，底部
/// 压进任务栏；内置 center() 以整块显示器为基准，同样会压入，故按
/// work_area 自行计算。
fn fit_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();
    let avail_w = work_area.size.width as f64 / scale;
    let avail_h = work_area.size.height as f64 / scale;
    // 默认尺寸取自 tauri.conf.json 的窗口配置，仅在超出工作区时收缩
    let (default_w, default_h) = app
        .config()
        .app
        .windows
        .first()
        .map(|w| (w.width, w.height))
        .unwrap_or((1280.0, 800.0));
    let width = default_w.min(avail_w);
    let height = default_h.min(avail_h);
    let _ = window.set_size(tauri::LogicalSize::new(width, height));
    let x = work_area.position.x
        + ((work_area.size.width as f64 - width * scale) / 2.0).round() as i32;
    let y = work_area.position.y
        + ((work_area.size.height as f64 - height * scale) / 2.0).round() as i32;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}
