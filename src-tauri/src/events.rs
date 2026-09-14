//! 后端事件推送：轮询 core 的下载队列/同步服务状态，emit 给前端。
//!
//! M1 占位：M3 填充 200ms 轮询任务（queue-status / sync-progress / log-append）。
