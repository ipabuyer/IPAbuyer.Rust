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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    /// 每个用例独立临时目录，避免 SQLite 句柄互扰。
    fn temp_state(name: &str) -> AppState {
        let dir = std::env::temp_dir().join(format!("ipabuyer-resolver-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        AppState::new(dir).expect("temp AppState")
    }

    fn write_custom_exe(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, b"MZ").unwrap();
        path
    }

    #[test]
    fn custom_flavor_with_valid_file_wins() {
        let state = temp_state("valid");
        let custom = write_custom_exe("resolver-custom-valid.exe");
        state
            .update_config(|c| {
                c.ipatool_flavor = "custom".into();
                c.custom_ipatool_path = Some(custom.to_string_lossy().into_owned());
            })
            .unwrap();

        assert_eq!(resolve_executable_path(&state), custom);
        let _ = std::fs::remove_file(custom);
    }

    #[test]
    fn main_flavor_ignores_custom_path() {
        let state = temp_state("main");
        let custom = write_custom_exe("resolver-custom-ignored.exe");
        state
            .update_config(|c| {
                c.custom_ipatool_path = Some(custom.to_string_lossy().into_owned());
            })
            .unwrap();

        assert_ne!(resolve_executable_path(&state), custom);
        let _ = std::fs::remove_file(custom);
    }

    #[test]
    fn custom_flavor_with_missing_file_falls_back() {
        let state = temp_state("missing");
        let missing = PathBuf::from("Z:/definitely/missing/ipatool.exe");
        state
            .update_config(|c| {
                c.ipatool_flavor = "custom".into();
                c.custom_ipatool_path = Some(missing.to_string_lossy().into_owned());
            })
            .unwrap();

        let resolved = resolve_executable_path(&state);
        assert_ne!(resolved, missing);
        // 回退到 bundled（存在时）或 PATH 兜底名
        assert!(resolved.is_file() || resolved == PathBuf::from("ipatool.exe"));
    }
}
