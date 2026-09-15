//! Windows 系统明暗主题监听。
//!
//! WebView2 的 prefers-color-scheme 不保证随系统主题实时更新，故由后端
//! 轮询注册表（AppsUseLightTheme），变化时设置窗口原生主题并 emit
//! `system-theme` 事件（"light"/"dark"），前端据此切换文档类。

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const APPS_USE_LIGHT: &str = "AppsUseLightTheme";

/// 读取"应用使用浅色主题"开关；None = 读取失败（如非 Windows）。
fn apps_use_light_theme() -> Option<bool> {
    let mut value: u32 = 0;
    let mut size = u32::try_from(std::mem::size_of::<u32>()).ok()?;
    let key = windows::core::HSTRING::from(PERSONALIZE_KEY);
    let name = windows::core::HSTRING::from(APPS_USE_LIGHT);
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &key,
            &name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as _),
            Some(&mut size),
        )
    };
    result.is_ok().then(|| value == 1)
}

fn theme_name(light: bool) -> &'static str {
    if light { "light" } else { "dark" }
}

/// 启动常驻监听：启动即广播一次（前端可能晚于事件注册，另有
/// `system_theme` 命令兜底查询），之后每秒轮询，变化才通知。
pub fn start_theme_watcher(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let mut last = apps_use_light_theme();
        if let Some(light) = last {
            let _ = app.emit("system-theme", theme_name(light));
        }
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let current = apps_use_light_theme();
            if current.is_none() || current == last {
                continue;
            }
            last = current;
            if let Some(light) = current {
                let theme = if light {
                    tauri::Theme::Light
                } else {
                    tauri::Theme::Dark
                };
                for window in app.webview_windows().values() {
                    let _ = window.set_theme(Some(theme));
                }
                let _ = app.emit("system-theme", theme_name(light));
            }
        }
    });
}

/// 当前系统主题（"light"/"dark"，未知时回退 "light"）。
pub fn current_system_theme() -> &'static str {
    theme_name(apps_use_light_theme().unwrap_or(true))
}
