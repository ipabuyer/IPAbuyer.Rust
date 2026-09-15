//! Tauri commands：前端 invoke 的后端入口。
//!
//! 返回给前端的本地化消息统一为 [`JsMessage`]（键名 + 参数或原文），
//! 由前端 i18next 渲染（键名约定与原 resw 一致）。

pub mod auth;
pub mod catalog;
pub mod ipatool;
pub mod purchases;
pub mod queue;
pub mod settings;
pub mod sync;

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
pub struct LogBuffer {
    pub entries: Mutex<Vec<LogEntryDto>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        LogBuffer {
            entries: Mutex::new(Vec::new()),
        }
    }

    pub fn push(&self, entry: LogEntryDto) {
        let mut guard = self.entries.lock().unwrap();
        if guard.len() >= 1000 {
            let overflow = guard.len() + 1 - 1000;
            guard.drain(..overflow);
        }
        guard.push(entry);
    }

    pub fn snapshot_from(&self, cursor: usize) -> Vec<LogEntryDto> {
        self.entries.lock().unwrap().iter().skip(cursor).cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

use std::sync::Mutex;
