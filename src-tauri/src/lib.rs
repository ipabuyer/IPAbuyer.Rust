//! IPAbuyer Tauri 后端。
//!
//! 分层：`core`（业务核心，并入自 IPAbuyer.Core）→ `state`/`commands`/`events`
//! （复刻原 C# 门面职责：配置存储、会话、队列/同步编排、日志缓冲、事件推送）。

pub mod core;
pub mod events;
pub mod state;

/// 构建 Tauri 应用并运行。
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
