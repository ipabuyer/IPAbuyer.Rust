//! 后端事件推送：200ms 轮询下载队列状态与日志缓冲增量，emit 给前端。
//!
//! 前端不自行轮询 core；状态变化经 `queue-status`、日志增量经 `log-append`。

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::commands::queue::{item_dto, QueueStatusDto};
use crate::state::AppState;

/// 在 setup 阶段启动常驻轮询任务。
pub fn start_polling(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut log_cursor: usize = 0;
        let mut last_queue_json = String::new();
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };

            // 队列状态：序列化比较，有变化才 emit
            let status = QueueStatusDto {
                running: state.queue.queue.is_running(),
                items: state
                    .queue
                    .queue
                    .items_snapshot()
                    .iter()
                    .map(item_dto)
                    .collect(),
            };
            if let Ok(json) = serde_json::to_string(&status) {
                if json != last_queue_json {
                    last_queue_json = json;
                    let _ = app.emit("queue-status", &status);
                }
            }

            // 日志增量
            let entries = state.log_buffer.snapshot_from(log_cursor);
            if !entries.is_empty() {
                log_cursor += entries.len();
                let _ = app.emit("log-append", &entries);
            }
        }
    });
}
