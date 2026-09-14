//! ipatool 命令结果。
//!
//! 移植自主仓库 `IpatoolResult`；本地化错误（超时等）按本地化原则
//! 以键名 + 参数（[`ErrorMessage`]）承载，宿主负责渲染。

use crate::core::ipatool::response_parser::NormalizedText;

/// 待宿主本地化的错误：稳定键名 + 格式化参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorMessage {
    pub key: &'static str,
    pub args: Vec<String>,
}

/// 一次 ipatool 命令的完整结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpatoolResult {
    pub output: NormalizedText,
    pub error: NormalizedText,
    /// 无法解析输出时的结构化错误（超时、缺少密钥等）；正常完成为 `None`。
    pub error_message: Option<ErrorMessage>,
    pub exit_code: i32,
    pub timed_out: bool,
}

impl IpatoolResult {
    /// 纯文本输出结果（正常完成路径）。
    pub fn from_streams(output: NormalizedText, error: NormalizedText, exit_code: i32) -> Self {
        Self {
            output,
            error,
            error_message: None,
            exit_code,
            timed_out: false,
        }
    }

    /// 用错误键构造结果（超时、缺少密钥等无可解析输出的场景）。
    pub fn from_error_message(message: ErrorMessage, exit_code: i32, timed_out: bool) -> Self {
        let ErrorMessage { key, args } = message;
        Self {
            output: NormalizedText::Raw(String::new()),
            error: NormalizedText::Keyed {
                key,
                args: args.clone(),
            },
            error_message: Some(ErrorMessage { key, args }),
            exit_code,
            timed_out,
        }
    }

    /// 输出与错误二选一：优先非空输出（对齐 C# `OutputOrError`）。
    pub fn output_or_error(&self) -> NormalizedText {
        if let NormalizedText::Raw(text) = &self.output {
            if !text.trim().is_empty() {
                return NormalizedText::Raw(text.clone());
            }
        }
        self.error.clone()
    }

    /// 原文视图（`Keyed` 贡献空字符串），供 JSON 解析器消费。
    pub fn output_or_error_raw(&self) -> String {
        match self.output_or_error() {
            NormalizedText::Raw(text) => text,
            NormalizedText::Keyed { .. } => String::new(),
        }
    }

    /// 未超时且退出码为零。
    pub fn is_success_response(&self) -> bool {
        !self.timed_out && self.exit_code == 0
    }

    /// 覆盖超时标志（测试与宿主构造场景用）。
    pub fn with_timed_out(mut self, timed_out: bool) -> Self {
        self.timed_out = timed_out;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_or_error_prefers_non_empty_output() {
        let result = IpatoolResult::from_streams(
            NormalizedText::Raw("{\"success\":true}".to_string()),
            NormalizedText::Raw("noise".to_string()),
            0,
        );

        assert_eq!(result.output_or_error_raw(), "{\"success\":true}");
        assert!(result.is_success_response());
    }

    #[test]
    fn output_or_error_falls_back_to_error() {
        let result = IpatoolResult::from_streams(
            NormalizedText::Raw(String::new()),
            NormalizedText::Raw("permission denied".to_string()),
            1,
        );

        assert_eq!(result.output_or_error_raw(), "permission denied");
        assert!(!result.is_success_response());
    }

    #[test]
    fn timed_out_result_carries_error_message() {
        let result = IpatoolResult::from_error_message(
            ErrorMessage {
                key: "Ipatool/Error/ExecutionTimeout",
                args: vec!["ipatool auth info".to_string()],
            },
            -1,
            true,
        );

        assert!(result.timed_out);
        assert_eq!(result.output_or_error_raw(), "");
        assert_eq!(
            result.error_message.expect("error message").key,
            "Ipatool/Error/ExecutionTimeout"
        );
    }
}
