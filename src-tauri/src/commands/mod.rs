//! Tauri commands：前端 invoke 的后端入口。
//!
//! 返回给前端的本地化消息统一为 [`JsMessage`]（键名 + 参数或原文），
//! 由前端 i18next 渲染（键名约定与原 resw 一致）。

pub mod auth;
pub mod settings;

use serde::Serialize;

use crate::core::auth::login::Message;
use crate::core::ipatool::response_parser::NormalizedText;

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
