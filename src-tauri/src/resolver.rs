//! ipatool.exe 路径解析（对齐 C# IpatoolPathResolver）。
//!
//! 优先级：自定义路径（flavor=custom 且路径有效）> 应用同目录 `ipatool.exe`
//! （MSIX 包内 / tauri build 输出目录）> PATH 兜底。

use std::path::PathBuf;

use crate::state::{AppState, IPATOOL_FLAVOR_CUSTOM};

/// 解析当前应使用的 ipatool 可执行文件路径。
pub fn resolve_executable_path(state: &AppState) -> PathBuf {
    let config = state.config.lock().unwrap();
    if config.ipatool_flavor.eq_ignore_ascii_case(IPATOOL_FLAVOR_CUSTOM) {
        if let Some(custom) = config.custom_ipatool_path.as_ref() {
            let path = PathBuf::from(custom);
            if path.is_file() {
                return path;
            }
        }
    }
    drop(config);

    // tauri build 会把 externalBin 复制到可执行文件同目录（Windows 下去掉 triple 后缀）
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("ipatool.exe");
            if bundled.is_file() {
                return bundled;
            }
        }
    }

    PathBuf::from("ipatool.exe")
}

/// 清理 ipatool 的 cookies.lock（登录异常时的修复手段，对齐 C# 行为）。
pub fn delete_cookie_lock_file() {
    if let Some(home) = dirs::home_dir() {
        let lock = home.join(".ipatool").join("cookies.lock");
        if lock.is_file() {
            let _ = std::fs::remove_file(lock);
        }
    }
}
