//! 下载队列命令：入队、启动（后台线程）、状态快照、队列级取消。
//!
//! 对齐原 C# DownloadQueueService 门面 + core ffi downloads 的编排：
//! 单飞行守卫由 core DownloadQueueService 维护；队列级取消（终止当前下载
//! 并结束本轮）；启动参数（exe 路径/输出目录/密钥/详细日志）在启动时读取。

use std::sync::atomic::AtomicBool;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::commands::LogEntryDto;
use crate::core::appcatalog::search_parser::SearchResult;
use crate::core::downloads::queue::{AddQueueResult, StartQueueParams};
use crate::core::downloads::DownloadQueueItem;
use crate::core::ipatool::client::IpatoolClient;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItemDto {
    pub bundle_id: String,
    pub app_id: String,
    pub name: String,
    pub developer: String,
    pub version: String,
    pub price: String,
    pub artwork_url: String,
    pub status: String,
    pub last_message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatusDto {
    pub running: bool,
    pub items: Vec<QueueItemDto>,
}

fn status_name(status: crate::core::downloads::DownloadQueueStatus) -> &'static str {
    use crate::core::downloads::DownloadQueueStatus as S;
    match status {
        S::Pending => "Pending",
        S::Downloading => "Downloading",
        S::Success => "Success",
        S::Failed => "Failed",
        S::Canceled => "Canceled",
    }
}

pub(crate) fn item_dto(item: &DownloadQueueItem) -> QueueItemDto {
    QueueItemDto {
        bundle_id: item.bundle_id.clone(),
        app_id: item.app_id.clone(),
        name: item.name.clone(),
        developer: item.developer.clone(),
        version: item.version.clone(),
        price: item.price.clone(),
        artwork_url: item.artwork_url.clone(),
        status: status_name(item.status).into(),
        last_message: item.last_message.clone(),
    }
}

/// 入队一个搜索结果条目，返回 Added / Updated / Requeued / Ignored。
#[tauri::command]
pub fn queue_add(
    state: State<'_, AppState>,
    bundle_id: String,
    app_id: Option<String>,
    name: Option<String>,
    developer: Option<String>,
    version: Option<String>,
    price: String,
    artwork_url: Option<String>,
) -> Result<String, String> {
    let search_result = SearchResult {
        bundle_id,
        id: app_id,
        name,
        developer,
        artwork_url,
        price,
        version,
        purchased: String::new(),
    };
    let result = state.queue.queue.add_or_update_from_search_result(
        &search_result,
        &mut |log| state.log_buffer.push((&log).into()),
        &mut || {},
    );
    Ok(match result {
        AddQueueResult::Added => "Added",
        AddQueueResult::Updated => "Updated",
        AddQueueResult::Requeued => "Requeued",
        AddQueueResult::Ignored => "Ignored",
    }
    .into())
}

/// 启动队列：参数在启动瞬间读取；已在运行时返回错误。
#[tauri::command]
pub fn queue_start(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if state.queue.queue.is_running() {
        return Err("队列已在运行".into());
    }

    let queue = std::sync::Arc::clone(&state.queue.queue);
    let cancel = std::sync::Arc::clone(&state.queue.cancel);
    cancel.store(false, std::sync::atomic::Ordering::Relaxed);

    let exe_path = crate::resolver::resolve_executable_path(&state);
    let passphrase = crate::state::get_passphrase().ok_or("缺少加密密钥，请先重新登录")?;
    let (output_directory, is_mock, detailed_log) = {
        let config = state.config.lock().unwrap();
        let mock = state.session.lock().unwrap().is_mock;
        (
            config.download_directory().to_string_lossy().into_owned(),
            mock,
            config.detailed_ipatool_log,
        )
    };

    // 状态/日志经 AppHandle emit，线程内不持有 State<'_>
    let app_for_log = app.clone();
    let app_for_change = app.clone();

    std::thread::spawn(move || {
        let client = IpatoolClient::new(exe_path);
        let runner = |item: &DownloadQueueItem,
                      on_chunk: Option<&(dyn std::ops::Fn(&str) + Sync)>,
                      cancel: &AtomicBool,
                      on_log: &mut dyn FnMut(crate::core::purchases::sync_service::LogMessage)|
         -> Result<crate::core::ipatool::result::IpatoolResult, crate::core::ipatool::client::ClientError> {
            let mut sink = |log: crate::core::purchases::sync_service::LogMessage| on_log(log);
            let detailed_sink: Option<&mut dyn FnMut(crate::core::purchases::sync_service::LogMessage)> =
                if detailed_log { Some(&mut sink) } else { None };
            client.download_app(
                &item.bundle_id,
                &output_directory,
                Some(&passphrase),
                on_chunk,
                cancel,
                detailed_sink,
            )
        };

        let mut on_log = |log: crate::core::purchases::sync_service::LogMessage| {
            app_for_log.emit("log-append", LogEntryDto::from(&log)).ok();
        };
        let mut on_change = || {
            app_for_change.emit("queue-changed", ()).ok();
        };

        queue.start_queue(StartQueueParams {
            output_directory: &output_directory,
            is_mock,
            detailed_log,
            cancel: &cancel,
            runner: &runner,
            on_log: &mut on_log,
            on_change: &mut on_change,
        });

        let _ = app_for_change.emit("queue-finished", ());
    });

    Ok(())
}

/// 状态快照（前端亦可经 queue-status 事件增量获取）。
#[tauri::command]
pub fn queue_status(state: State<'_, AppState>) -> QueueStatusDto {
    QueueStatusDto {
        running: state.queue.queue.is_running(),
        items: state
            .queue
            .queue
            .items_snapshot()
            .iter()
            .map(item_dto)
            .collect(),
    }
}

/// 队列级取消：终止当前下载并结束本轮。
#[tauri::command]
pub fn queue_cancel_current(state: State<'_, AppState>) {
    state
        .queue
        .cancel
        .store(true, std::sync::atomic::Ordering::Relaxed);
}

/// 清空日志缓冲。
#[tauri::command]
pub fn logs_clear(state: State<'_, AppState>) {
    state.log_buffer.clear();
}

/// 全量日志快照（初始化日志面板用）。
#[tauri::command]
pub fn logs_snapshot(state: State<'_, AppState>) -> Vec<LogEntryDto> {
    state.log_buffer.snapshot_from(0)
}
