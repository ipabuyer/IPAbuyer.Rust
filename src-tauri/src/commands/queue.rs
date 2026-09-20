//! 下载队列命令：入队、启动（后台线程）、状态快照、队列级取消。
//!
//! 对齐原 C# DownloadQueueService 门面 + core ffi downloads 的编排：
//! 单飞行守卫由 core DownloadQueueService 维护；队列级取消（终止当前下载
//! 并结束本轮）；启动参数（exe 路径/输出目录/密钥/详细日志）在启动时读取。

use std::sync::atomic::AtomicBool;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

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
    pub platform: String,
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
        platform: item.platform.clone(),
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
    platform: Option<String>,
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
        platform: crate::core::platform::normalize(
            platform.as_deref().unwrap_or(crate::core::platform::IOS),
        )
        .to_string(),
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
    let lang = crate::i18n::Lang::from_state(&state);
    if state.queue.queue.is_running() {
        return Err(lang.message("error-queue-already-running"));
    }

    let queue = std::sync::Arc::clone(&state.queue.queue);
    let cancel = std::sync::Arc::clone(&state.queue.cancel);
    cancel.store(false, std::sync::atomic::Ordering::Relaxed);

    let exe_path = crate::resolver::resolve_executable_path(&state);
    let passphrase = crate::state::get_passphrase()
        .ok_or_else(|| lang.message("error-missing-passphrase"))?;
    let (output_directory, is_mock, detailed_log) = {
        let config = state.config.lock().unwrap();
        let mock = state.session.lock().unwrap().is_mock;
        (
            config.download_directory().to_string_lossy().into_owned(),
            mock,
            config.detailed_ipatool_log,
        )
    };

    // 状态经 AppHandle emit；日志统一写入全局缓冲，由轮询任务增量推送
    // （直接 emit 单条会破坏前端按数组解析 log-append 的约定）。
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
                &item.platform,
            )
        };

        let mut on_log = |log: crate::core::purchases::sync_service::LogMessage| {
            if let Some(state) = app_for_log.try_state::<AppState>() {
                state.log_buffer.push((&log).into());
            }
        };
        let mut on_change = || {
            app_for_change.emit("queue-changed", ()).ok();
        };

        queue.start_queue(StartQueueParams {
            output_directory: &output_directory,
            is_mock,
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
    state.log_buffer.snapshot_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(status: crate::core::downloads::DownloadQueueStatus) -> DownloadQueueItem {
        let mut item = DownloadQueueItem::new("com.example.app", "ios");
        item.status = status;
        item
    }

    #[test]
    fn status_name_covers_all_variants() {
        use crate::core::downloads::DownloadQueueStatus as S;
        assert_eq!(status_name(S::Pending), "Pending");
        assert_eq!(status_name(S::Downloading), "Downloading");
        assert_eq!(status_name(S::Success), "Success");
        assert_eq!(status_name(S::Failed), "Failed");
        assert_eq!(status_name(S::Canceled), "Canceled");
    }

    #[test]
    fn item_dto_maps_fields_and_serializes_camel_case() {
        let mut queue_item = DownloadQueueItem::new("com.example.app", "macos");
        queue_item.app_id = "42".into();
        queue_item.name = "Example".into();
        queue_item.developer = "Dev".into();
        queue_item.version = "1.2.3".into();
        queue_item.price = "free".into();
        queue_item.artwork_url = "https://a/42.png".into();
        queue_item.status = crate::core::downloads::DownloadQueueStatus::Success;
        queue_item.last_message = "done".into();

        let dto = item_dto(&queue_item);
        assert_eq!(dto.bundle_id, "com.example.app");
        assert_eq!(dto.platform, "macos");
        assert_eq!(dto.app_id, "42");
        assert_eq!(dto.status, "Success");
        assert_eq!(dto.last_message, "done");

        // serde 对前端的字段名为 camelCase
        let json = serde_json::to_value(&dto).unwrap();
        assert!(json.get("bundleId").is_some());
        assert!(json.get("artworkUrl").is_some());
        assert!(json.get("lastMessage").is_some());
        assert!(json.get("bundle_id").is_none());
    }
}
