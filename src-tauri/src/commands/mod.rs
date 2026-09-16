//! Tauri commands：前端 invoke 的后端入口。
//!
//! 返回给前端的本地化消息统一为 [`JsMessage`]（键名 + 参数或原文），
//! 由前端 i18next 渲染（键名约定与原 resw 一致）。

pub mod auth;
pub mod catalog;
pub mod filter;
pub mod ipatool;
pub mod logs;
pub mod purchases;
pub mod queue;
pub mod settings;
pub mod sync;
pub mod theme;

use serde::Serialize;

use crate::core::auth::login::Message;
use crate::core::ipatool::response_parser::NormalizedText;
use crate::core::purchases::sync_service::{LogLevel, LogMessage};

/// 前端消息：`{ type: "key", key, args }` 或 `{ type: "raw", text }`。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JsMessage {
    Key { key: String, args: Vec<String> },
    Raw { text: String },
}

impl JsMessage {
    pub fn key(key: &str, args: &[String]) -> Self {
        JsMessage::Key {
            key: key.into(),
            args: args.into(),
        }
    }

    pub fn raw(text: impl Into<String>) -> Self {
        JsMessage::Raw { text: text.into() }
    }
}

impl From<&Message> for JsMessage {
    fn from(message: &Message) -> Self {
        match message {
            Message::Key { key, args } => JsMessage::key(key, args),
            Message::Raw(text) => JsMessage::raw(text),
        }
    }
}

impl From<&NormalizedText> for JsMessage {
    fn from(text: &NormalizedText) -> Self {
        match text {
            NormalizedText::Raw(text) => JsMessage::raw(text),
            NormalizedText::Keyed { key, args } => JsMessage::key(key, args),
        }
    }
}

/// 结构化日志条目（键名消息保留多语言渲染能力，前端负责渲染）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntryDto {
    pub timestamp: String,
    pub level: String,
    pub message: JsMessage,
}

impl LogEntryDto {
    pub fn new(level: &str, message: JsMessage) -> Self {
        LogEntryDto {
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            level: level.into(),
            message,
        }
    }
}

pub fn log_level_name(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Info => "info",
        LogLevel::Tip => "tip",
        LogLevel::Success => "success",
        LogLevel::Error => "error",
        LogLevel::Ipatool => "ipatool",
    }
}

impl From<&LogMessage> for LogEntryDto {
    fn from(log: &LogMessage) -> Self {
        LogEntryDto::new(log_level_name(log.level), (&log.message).into())
    }
}

/// 应用日志缓冲（环形，上限对齐原 UiLogStore 1000 行）。
///
/// 每条带单调递增序号：增量拉取按序号过滤，清空（保留计数）与环形挤出
/// 都不影响游标有效性——按数组下标 skip 的游标会在清空后失步、在填满
/// 1000 条后永久卡死。
pub struct LogBuffer {
    entries: Mutex<Vec<BufferedLog>>,
    total: AtomicUsize,
}

struct BufferedLog {
    seq: usize,
    entry: LogEntryDto,
}

impl LogBuffer {
    pub fn new() -> Self {
        LogBuffer {
            entries: Mutex::new(Vec::new()),
            total: AtomicUsize::new(0),
        }
    }

    pub fn push(&self, entry: LogEntryDto) {
        let seq = self.total.fetch_add(1, Ordering::Relaxed) + 1;
        let mut guard = self.entries.lock().unwrap();
        if guard.len() >= 1000 {
            let overflow = guard.len() + 1 - 1000;
            guard.drain(..overflow);
        }
        guard.push(BufferedLog { seq, entry });
    }

    /// 取序号大于 cursor 的增量，返回条目与新游标。
    pub fn snapshot_since(&self, cursor: usize) -> (Vec<LogEntryDto>, usize) {
        let guard = self.entries.lock().unwrap();
        let mut latest = cursor;
        let entries = guard
            .iter()
            .filter(|buffered| buffered.seq > cursor)
            .map(|buffered| {
                latest = latest.max(buffered.seq);
                buffered.entry.clone()
            })
            .collect();
        (entries, latest)
    }

    /// 全量快照（日志窗口初始化用）。
    pub fn snapshot_all(&self) -> Vec<LogEntryDto> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|buffered| buffered.entry.clone())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    /// 只清条目；序号计数单调递增，已发放的游标不失效。
    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::purchases::sync_service::{LogLevel, LogMessage};

    #[test]
    fn js_message_key_serializes_camel_case_tag() {
        let message = JsMessage::key("Some/Key", &["a".into(), "b".into()]);
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["type"], "key");
        assert_eq!(json["key"], "Some/Key");
        assert_eq!(json["args"][0], "a");
        assert_eq!(json["args"][1], "b");
    }

    #[test]
    fn js_message_raw_serializes_text_only() {
        let message = JsMessage::raw("原文");
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(json["type"], "raw");
        assert_eq!(json["text"], "原文");
    }

    #[test]
    fn log_entry_dto_from_log_message_maps_level() {
        let log = LogMessage::key(LogLevel::Success, "Some/Success", Vec::new());
        let entry = LogEntryDto::from(&log);
        assert_eq!(entry.level, "success");
        assert!(matches!(entry.message, JsMessage::Key { .. }));
    }

    #[test]
    fn log_level_name_covers_all_levels() {
        assert_eq!(log_level_name(LogLevel::Info), "info");
        assert_eq!(log_level_name(LogLevel::Tip), "tip");
        assert_eq!(log_level_name(LogLevel::Success), "success");
        assert_eq!(log_level_name(LogLevel::Error), "error");
        assert_eq!(log_level_name(LogLevel::Ipatool), "ipatool");
    }

    #[test]
    fn log_buffer_is_bounded_ring() {
        let buffer = LogBuffer::new();
        for i in 0..1005 {
            buffer.push(LogEntryDto::new(
                "info",
                JsMessage::key(&format!("K/{i}"), &[]),
            ));
        }
        assert_eq!(buffer.len(), 1000);
        let snapshot = buffer.snapshot_all();
        assert_eq!(snapshot.len(), 1000);
        // 最早的 5 条被挤出
        assert!(matches!(&snapshot[0].message, JsMessage::Key { key, .. } if key == "K/5"));
        assert!(matches!(&snapshot[999].message, JsMessage::Key { key, .. } if key == "K/1004"));
    }

    #[test]
    fn log_buffer_snapshot_since_returns_increment() {
        let buffer = LogBuffer::new();
        buffer.push(LogEntryDto::new("info", JsMessage::raw("a")));
        buffer.push(LogEntryDto::new("info", JsMessage::raw("b")));

        let (first, cursor) = buffer.snapshot_since(0);
        assert_eq!(first.len(), 2);
        assert_eq!(cursor, 2);

        buffer.push(LogEntryDto::new("info", JsMessage::raw("c")));
        let (increment, cursor) = buffer.snapshot_since(cursor);
        assert_eq!(increment.len(), 1);
        assert!(matches!(&increment[0].message, JsMessage::Raw { text } if text == "c"));
        assert_eq!(cursor, 3);

        buffer.clear();
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn log_buffer_cursor_survives_clear_and_ring_eviction() {
        let buffer = LogBuffer::new();
        for i in 0..1005 {
            buffer.push(LogEntryDto::new(
                "info",
                JsMessage::key(&format!("K/{i}"), &[]),
            ));
        }
        let (_, cursor) = buffer.snapshot_since(0);
        assert_eq!(cursor, 1005);

        // 清空后游标依然有效：新日志立即可见（下标制游标会 skip 越界永久卡死）
        buffer.clear();
        buffer.push(LogEntryDto::new("info", JsMessage::raw("fresh")));
        let (entries, cursor) = buffer.snapshot_since(cursor);
        assert_eq!(entries.len(), 1);
        assert!(matches!(&entries[0].message, JsMessage::Raw { text } if text == "fresh"));
        assert_eq!(cursor, 1006);

        // 环形挤出后游标不回退、不重复
        buffer.push(LogEntryDto::new("info", JsMessage::raw("next")));
        let (entries, cursor) = buffer.snapshot_since(cursor);
        assert_eq!(entries.len(), 1);
        assert_eq!(cursor, 1007);
    }
}
