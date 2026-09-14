//! 全局日志缓冲（移植自主仓库 `UiLogStore`/`UiLogFormatter`）。
//!
//! 环形缓冲上限 1000 条，超出后丢弃最旧条目；条目在写入时即格式化为
//! `[yyyy-MM-dd HH:mm:ss] [TAG] 消息`。颜色渲染属宿主职责，Core 只提供
//! 等级与原文。

use chrono::Local;
use std::sync::Mutex;

/// 缓冲上限（对齐 C# `MaxLogLines`）。
pub const MAX_LOG_LINES: usize = 1000;

/// 日志等级（对齐 C# `UiLogLevel`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLogLevel {
    Info,
    Tip,
    Success,
    Error,
    Ipatool,
}

/// 日志来源（对齐 C# `UiLogSource`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLogSource {
    Auto,
    App,
    Ipatool,
}

/// 已格式化的日志条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiLogEntry {
    pub formatted_text: String,
    pub level: UiLogLevel,
}

/// 未格式化的日志消息（宿主中转用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiLogMessage {
    pub message: String,
    pub level: UiLogLevel,
    pub source: UiLogSource,
}

/// 格式化日志：`[本地时间] [TAG] 原文`；TAG 随等级变化（ipatool 输出为小写 `ipatool`）。
pub fn build(message: Option<&str>, level: UiLogLevel) -> UiLogEntry {
    let raw = message.unwrap_or("").trim();
    let tag = match level {
        UiLogLevel::Tip => "TIP",
        UiLogLevel::Success => "SUCCESS",
        UiLogLevel::Error => "ERROR",
        UiLogLevel::Ipatool => "ipatool",
        UiLogLevel::Info => "INFO",
    };
    let formatted_text = format!(
        "[{}] [{tag}] {raw}",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    UiLogEntry {
        formatted_text,
        level,
    }
}

static ENTRIES: Mutex<Vec<UiLogEntry>> = Mutex::new(Vec::new());

/// 追加一条日志；超过上限时丢弃最旧条目。
pub fn append(message: &str, level: UiLogLevel) -> UiLogEntry {
    let entry = build(Some(message), level);
    let mut entries = ENTRIES.lock().expect("log store lock");
    entries.push(entry.clone());
    if entries.len() > MAX_LOG_LINES {
        entries.remove(0);
    }
    entry
}

/// 追加一条带来源的消息（来源仅透传给宿主，条目本身不携带）。
pub fn append_message(message: &UiLogMessage) -> UiLogEntry {
    append(&message.message, message.level)
}

/// 全部条目快照。
pub fn snapshot() -> Vec<UiLogEntry> {
    ENTRIES.lock().expect("log store lock").clone()
}

/// 全部条目按行拼接的文本。
pub fn text() -> String {
    let entries = ENTRIES.lock().expect("log store lock");
    let mut text = String::new();
    for entry in entries.iter() {
        text.push_str(&entry.formatted_text);
        text.push('\n');
    }
    text
}

/// 清空缓冲。
pub fn clear() {
    ENTRIES.lock().expect("log store lock").clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    // 日志缓冲是进程级单例：串行化日志测试，避免并行断言互相干扰。
    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn build_formats_entry_with_tag_and_timestamp() {
        let entry = build(Some("  hello  "), UiLogLevel::Tip);
        assert!(entry.formatted_text.ends_with("] [TIP] hello"));
        assert_eq!(entry.level, UiLogLevel::Tip);

        let entry = build(Some("x"), UiLogLevel::Ipatool);
        assert!(entry.formatted_text.contains("[ipatool] x"));

        let entry = build(Some("x"), UiLogLevel::Error);
        assert!(entry.formatted_text.contains("[ERROR] x"));

        let entry = build(Some("x"), UiLogLevel::Success);
        assert!(entry.formatted_text.contains("[SUCCESS] x"));

        let entry = build(Some("x"), UiLogLevel::Info);
        assert!(entry.formatted_text.contains("[INFO] x"));

        let entry = build(None, UiLogLevel::Info);
        assert!(entry.formatted_text.ends_with("[INFO] "));
    }

    #[test]
    fn store_appends_snapshots_and_clears() {
        let _guard = test_guard();
        clear();
        append("first", UiLogLevel::Info);
        append("second", UiLogLevel::Error);

        let entries = snapshot();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].formatted_text.ends_with("first"));
        assert_eq!(entries[1].level, UiLogLevel::Error);
        assert!(text().contains("second"));

        clear();
        assert!(snapshot().is_empty());
    }

    #[test]
    fn store_drops_oldest_beyond_limit() {
        let _guard = test_guard();
        clear();
        for index in 0..(MAX_LOG_LINES + 50) {
            append(&index.to_string(), UiLogLevel::Info);
        }

        let entries = snapshot();
        assert_eq!(entries.len(), MAX_LOG_LINES);
        assert!(entries[0].formatted_text.ends_with("50"));
        assert!(
            entries
                .last()
                .unwrap()
                .formatted_text
                .ends_with(&(MAX_LOG_LINES + 49).to_string())
        );
        clear();
    }
}
