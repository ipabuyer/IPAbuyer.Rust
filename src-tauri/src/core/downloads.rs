//! 下载域：队列条目、输出解析、结果解析与队列服务。

pub mod output_parser;
pub mod queue;
pub mod result_parser;

use chrono::{DateTime, Local};

/// 下载队列状态（对齐 C# `DownloadQueueStatus`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadQueueStatus {
    Pending,
    Downloading,
    Success,
    Failed,
    Canceled,
}

/// 下载队列条目（移植自 C# `DownloadQueueItem`；INotifyPropertyChanged 属
/// 宿主 UI 职责，`last_message` 按本地化原则存储 resw 键名，宿主负责渲染）。
/// `platform` 区分同一 bundleId 的 iOS/Mac 条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadQueueItem {
    pub bundle_id: String,
    pub platform: String,
    pub app_id: String,
    pub name: String,
    pub developer: String,
    pub version: String,
    pub price: String,
    pub artwork_url: String,
    pub added_at: DateTime<Local>,
    pub status: DownloadQueueStatus,
    pub last_message: String,
}

impl DownloadQueueItem {
    pub fn new(bundle_id: impl Into<String>, platform: impl Into<String>) -> Self {
        let platform = platform.into();
        Self {
            bundle_id: bundle_id.into(),
            platform: crate::core::platform::normalize(&platform).to_string(),
            app_id: String::new(),
            name: String::new(),
            developer: String::new(),
            version: String::new(),
            price: String::new(),
            artwork_url: String::new(),
            added_at: Local::now(),
            status: DownloadQueueStatus::Pending,
            last_message: "DownloadQueue/Status/Pending".to_string(),
        }
    }
}
