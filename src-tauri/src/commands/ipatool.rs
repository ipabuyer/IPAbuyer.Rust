//! ipatool 管理命令：来源切换、自定义路径、导出内置版本、清空 ipatool 数据、
//! 旧版 WinUI3 数据库检测与导入。

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::state::{AppState, IPATOOL_FLAVOR_CUSTOM, IPATOOL_FLAVOR_MAIN};

pub const BUILTIN_IPATOOL_VERSION: &str = "2.5.0";
const LEGACY_PACKAGE_FAMILY: &str = "IPAbuyer.IPAbuyer_kr1hdvrv6tpd0";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpatoolInfo {
    pub flavor: String,
    pub custom_path: Option<String>,
    pub builtin_version: String,
    /// 当前实际生效的可执行文件路径（自定义失效时回退内置）。
    pub active_path: String,
    /// 内置 ipatool.exe 是否可用（同目录存在）。
    pub builtin_available: bool,
}

fn bundled_ipatool_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let path = dir.join("ipatool.exe");
    path.is_file().then_some(path)
}

#[tauri::command]
pub fn ipatool_info(state: State<'_, AppState>) -> IpatoolInfo {
    let flavor = state.config.lock().unwrap().ipatool_flavor.clone();
    let custom_path = state.config.lock().unwrap().custom_ipatool_path.clone();
    let custom_usable = flavor.eq_ignore_ascii_case(IPATOOL_FLAVOR_CUSTOM)
        && custom_path
            .as_ref()
            .is_some_and(|p| PathBuf::from(p).is_file());
    let active_path = if custom_usable {
        custom_path.clone().unwrap()
    } else {
        bundled_ipatool_path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "ipatool.exe".into())
    };
    IpatoolInfo {
        flavor,
        custom_path,
        builtin_version: BUILTIN_IPATOOL_VERSION.into(),
        active_path,
        builtin_available: bundled_ipatool_path().is_some(),
    }
}

#[tauri::command]
pub fn ipatool_set_flavor(state: State<'_, AppState>, flavor: String) -> Result<(), String> {
    if flavor != IPATOOL_FLAVOR_MAIN && flavor != IPATOOL_FLAVOR_CUSTOM {
        return Err(format!("无效的 ipatool 来源: {flavor}"));
    }
    state.update_config(|c| c.ipatool_flavor = flavor)?;
    Ok(())
}

#[tauri::command]
pub fn ipatool_set_custom_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let trimmed = path.trim().to_string();
    if !trimmed.to_lowercase().ends_with(".exe") {
        return Err("自定义 ipatool 必须是 .exe 文件".into());
    }
    if !PathBuf::from(&trimmed).is_file() {
        return Err(format!("文件不存在: {trimmed}"));
    }
    state.update_config(|c| {
        c.custom_ipatool_path = Some(trimmed.clone());
        c.ipatool_flavor = IPATOOL_FLAVOR_CUSTOM.into();
    })?;
    Ok(())
}

/// 删除自定义插槽（只移除配置，不删除原文件），并回退到内置来源。
#[tauri::command]
pub fn ipatool_delete_custom(state: State<'_, AppState>) -> Result<(), String> {
    state.update_config(|c| {
        c.custom_ipatool_path = None;
        c.ipatool_flavor = IPATOOL_FLAVOR_MAIN.into();
    })?;
    Ok(())
}

/// 导出当前生效的 ipatool.exe 到下载目录，目标文件名 `ipatool.exe`（覆盖确认由前端处理）。
#[tauri::command]
pub fn ipatool_export(state: State<'_, AppState>) -> Result<String, String> {
    let source = crate::resolver::resolve_executable_path(&state);
    if !source.is_file() {
        return Err("内置 ipatool.exe 不存在".into());
    }
    let target = state.config.lock().unwrap().download_directory().join("ipatool.exe");
    std::fs::copy(&source, &target).map_err(|e| format!("导出失败: {e}"))?;
    Ok(target.to_string_lossy().into_owned())
}

/// 清空 ipatool 数据目录（`~/.ipatool/`）。
#[tauri::command]
pub fn ipatool_clear_data() -> Result<(), String> {
    let dir = dirs::home_dir()
        .ok_or("无法解析用户目录")?
        .join(".ipatool");
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("清空失败: {e}"))?;
    }
    Ok(())
}

/// 旧版 WinUI3 数据库是否存在（包 LocalState 下同名 schema 数据库）。
#[tauri::command]
pub fn legacy_db_exists() -> bool {
    legacy_db_path().is_some_and(|p| p.is_file())
}

/// 导入旧版数据库：关闭当前连接后覆盖，再重新打开（schema 相同，由 core 迁移）。
#[tauri::command]
pub fn legacy_db_import(state: State<'_, AppState>) -> Result<(), String> {
    let source = legacy_db_path().ok_or("未找到旧版数据库")?;
    {
        let mut db = state.db.lock().unwrap();
        *db = None; // 释放文件句柄
    }
    let target = state.db_path().clone();
    std::fs::copy(&source, &target).map_err(|e| format!("导入失败: {e}"))?;
    let db = crate::core::db::PurchasedAppsDb::open(&target)
        .map_err(|e| format!("重新打开数据库失败: {e}"))?;
    *state.db.lock().unwrap() = Some(db);
    Ok(())
}

fn legacy_db_path() -> Option<PathBuf> {
    let local = dirs::data_local_dir()?;
    Some(
        local
            .join("Packages")
            .join(LEGACY_PACKAGE_FAMILY)
            .join("LocalState")
            .join("PurchasedAppDb.db"),
    )
}
