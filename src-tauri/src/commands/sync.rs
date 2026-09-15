//! 已购列表全量同步命令：后台线程执行 core PurchaseSyncService，进度经事件推送。
//!
//! 取消经共享 `cancel` 标志（`sync_cancel` 命令置位）；完成/取消时 emit
//! `sync-progress`（running=false）收尾。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::commands::LogEntryDto;
use crate::core::purchases::sync_service::{PurchaseSyncService, SyncOutcome};
use crate::core::ipatool::client::IpatoolClient;
use crate::state::AppState;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub running: bool,
    pub synced: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResultDto {
    /// Completed / Canceled / InvalidAccount / Failed / Mock / AlreadyRunning
    pub outcome: String,
    pub synced: i64,
    pub total: i64,
    pub message: Option<String>,
}

/// 启动全量同步（阻塞至完成或取消；前端在异步上下文调用）。
/// 未登录返回 InvalidAccount；测试账户返回 Mock（不执行）；进行中返回 AlreadyRunning。
#[tauri::command]
pub fn sync_start(app: AppHandle, state: State<'_, AppState>) -> Result<SyncResultDto, String> {
    let (account, is_mock, logged_in) = {
        let session = state.session.lock().unwrap();
        (
            session.account.clone(),
            session.is_mock,
            session.logged_in,
        )
    };
    if !logged_in {
        return Ok(SyncResultDto {
            outcome: "InvalidAccount".into(),
            synced: 0,
            total: 0,
            message: Some("未登录".into()),
        });
    }
    if is_mock {
        return Ok(SyncResultDto {
            outcome: "Mock".into(),
            synced: 0,
            total: 0,
            message: None,
        });
    }
    if state.sync.service.is_running() {
        return Ok(SyncResultDto {
            outcome: "AlreadyRunning".into(),
            synced: 0,
            total: 0,
            message: None,
        });
    }

    let account = account.ok_or("未登录")?;
    let account_for_record = account.clone();
    let passphrase = crate::state::get_passphrase().ok_or("缺少加密密钥，请先重新登录")?;
    let exe_path = crate::resolver::resolve_executable_path(&state);
    let detailed_log = state.config.lock().unwrap().detailed_ipatool_log;

    // 重置共享取消标志
    state.sync.cancel.store(false, Ordering::Relaxed);
    let cancel = Arc::clone(&state.sync.cancel);

    let mut db_guard = state.db.lock().unwrap();
    let db = db_guard.as_mut().ok_or("数据库未初始化")?;
    let service = &state.sync.service;
    let app_for_log = app.clone();
    let app_for_progress = app.clone();

    // MutexGuard 不能跨线程，用 thread::scope 绑定 db 借用生命周期（同步为阻塞调用）
    let outcome: SyncOutcome = std::thread::scope(|scope| {
        scope
            .spawn(move || {
                let client = IpatoolClient::new(exe_path);
                let mut on_progress = |synced: i64, total: i64| {
                    let _ = app_for_progress.emit(
                        "sync-progress",
                        SyncProgress { running: true, synced, total },
                    );
                };
                let mut on_log = |log: crate::core::purchases::sync_service::LogMessage| {
                    let _ = app_for_log.emit("log-append", LogEntryDto::from(&log));
                };
                service.sync(
                    &account,
                    Some(&passphrase),
                    &client,
                    db,
                    &cancel,
                    detailed_log,
                    &mut on_progress,
                    &mut on_log,
                )
            })
            .join()
            .unwrap_or(SyncOutcome::Failed { message: "同步线程崩溃".into() })
    });

    let _ = db_guard
        .as_ref()
        .ok_or("数据库未初始化")?
        .record_sync_attempt(&account_for_record, matches!(outcome, SyncOutcome::Completed { .. }));

    let dto = |outcome_name: &str, synced: i64, total: i64, message: Option<String>| SyncResultDto {
        outcome: outcome_name.into(),
        synced,
        total,
        message,
    };

    Ok(match outcome {
        SyncOutcome::Completed { synced, total } => {
            let _ = app.emit("sync-progress", SyncProgress { running: false, synced, total });
            dto("Completed", synced, total, None)
        }
        SyncOutcome::Canceled => {
            let _ = app.emit("sync-progress", SyncProgress { running: false, synced: 0, total: 0 });
            dto("Canceled", 0, 0, None)
        }
        SyncOutcome::InvalidAccount => dto("InvalidAccount", 0, 0, Some("未登录".into())),
        SyncOutcome::AlreadyRunning => dto("AlreadyRunning", 0, 0, None),
        SyncOutcome::Failed { message } => {
            let _ = app.emit("sync-progress", SyncProgress { running: false, synced: 0, total: 0 });
            dto("Failed", 0, 0, Some(message))
        }
    })
}

/// 取消进行中的同步。
#[tauri::command]
pub fn sync_cancel(state: State<'_, AppState>) {
    state.sync.cancel.store(true, Ordering::Relaxed);
}

/// 当前同步运行状态。
#[tauri::command]
pub fn sync_status(state: State<'_, AppState>) -> SyncProgress {
    SyncProgress {
        running: state.sync.service.is_running(),
        synced: 0,
        total: 0,
    }
}

/// 上次成功同步时间（ISO 字符串或 null）。
#[tauri::command]
pub fn sync_last_time(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let account = state
        .session
        .lock()
        .unwrap()
        .account
        .clone()
        .ok_or("未登录")?;
    let db = state.db.lock().unwrap();
    let db = db.as_ref().ok_or("数据库未初始化")?;
    db.get_last_successful_sync_utc(&account).map_err(|e| e.to_string())
}
