//! ipatool 响应解析。
//!
//! 移植自主仓库 `IpatoolResponseParser`。按本地化原则（references/localization.md），
//! 原实现中经资源加载器格式化的可读错误改为返回稳定键名 + 参数
//! （[`NormalizedText::Keyed`]），由宿主用 `.resw` 渲染。

use std::sync::OnceLock;

use crate::core::json;
use regex::Regex;

/// 归一化后的流文本：`Raw` 为原文，`Keyed` 为待宿主本地化的键与参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedText {
    Raw(String),
    Keyed {
        key: &'static str,
        args: Vec<String>,
    },
}

impl NormalizedText {
    /// 宿主本地化用：键名。
    pub fn key(&self) -> &str {
        match self {
            NormalizedText::Raw(_) => "",
            NormalizedText::Keyed { key, .. } => key,
        }
    }

    /// 宿主本地化用：格式化参数。
    pub fn args(&self) -> &[String] {
        match self {
            NormalizedText::Raw(_) => &[],
            NormalizedText::Keyed { args, .. } => args,
        }
    }

    /// 原文视图；`Keyed` 变体为空字符串。
    pub fn as_raw(&self) -> &str {
        match self {
            NormalizedText::Raw(text) => text,
            NormalizedText::Keyed { .. } => "",
        }
    }
}

/// 归一化后的标准输出/标准错误对。
#[derive(Debug, PartialEq, Eq)]
pub struct NormalizedStreams {
    pub output: NormalizedText,
    pub error: NormalizedText,
}

/// 从 payload 提取账户邮箱：优先读 JSON 的 `email`/`eamil`（历史拼写别名）属性，退化为正则扫描。
pub fn extract_email(payload: Option<&str>) -> String {
    let Some(payload) = payload else {
        return String::new();
    };
    if payload.trim().is_empty() {
        return String::new();
    }

    for token in json::enumerate_tokens(Some(payload)) {
        if let Some(email) = json::try_read_string(&token, &["email", "eamil"]) {
            if !email.trim().is_empty() {
                return email.trim().to_string();
            }
        }
    }

    email_regex()
        .find(payload)
        .map(|matched| matched.as_str().to_string())
        .unwrap_or_default()
}

/// 判断 payload 是否表达成功：JSON `success` 为真，或文本包含 `success=true` / `"success":true`。
pub fn is_success(payload: Option<&str>) -> bool {
    let Some(payload) = payload else {
        return false;
    };

    json::enumerate_tokens(Some(payload))
        .iter()
        .any(|token| json::try_read_boolean(token, "success") == Some(true))
        || (!payload.trim().is_empty()
            && (contains_ignore_case(payload, "success=true")
                || contains_ignore_case(payload, "\"success\":true")))
}

/// 判断 payload 是否带显式失败标志：JSON `success` 为假，或文本包含 `success=false` / `"success":false`。
pub fn has_explicit_failure(payload: Option<&str>) -> bool {
    let Some(payload) = payload else {
        return false;
    };

    json::enumerate_tokens(Some(payload))
        .iter()
        .any(|token| json::try_read_boolean(token, "success") == Some(false))
        || (!payload.trim().is_empty()
            && (contains_ignore_case(payload, "success=false")
                || contains_ignore_case(payload, "\"success\":false")))
}

/// 判断是否为"账户不存在于 keyring"错误（需同时命中两个已知短语）。
pub fn is_account_missing_from_keyring(payload: Option<&str>) -> bool {
    let Some(payload) = payload else {
        return false;
    };
    if payload.trim().is_empty() {
        return false;
    }

    contains_ignore_case(payload, "failed to get account")
        && contains_ignore_case(payload, "could not be found in the keyring")
}

/// 归一化标准输出/标准错误：优先保留 JSON 内容；输出为空时用错误流兜底，
/// 错误流为空且退出码非零时用输出兜底；兜底内容无法解释时返回带退出码的键名。
pub fn normalize_streams(
    stdout: Option<&str>,
    stderr: Option<&str>,
    exit_code: i32,
) -> NormalizedStreams {
    let output_text = stdout.unwrap_or("").trim();
    let error_text = stderr.unwrap_or("").trim();
    let normalized_output =
        extract_meaningful_json(output_text).unwrap_or_else(|| output_text.to_string());
    let normalized_error =
        extract_meaningful_json(error_text).unwrap_or_else(|| error_text.to_string());

    let output = if normalized_output.is_empty() {
        build_readable_error(&normalized_error, exit_code)
    } else {
        NormalizedText::Raw(normalized_output.clone())
    };

    let error = if normalized_error.is_empty() && exit_code != 0 {
        build_readable_error(&normalized_output, exit_code)
    } else {
        NormalizedText::Raw(normalized_error)
    };

    NormalizedStreams { output, error }
}

fn extract_meaningful_json(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Some(trimmed.to_string());
    }

    let json_lines: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && (line.starts_with('{') || line.starts_with('[')))
        .collect();
    if json_lines.is_empty() {
        None
    } else {
        Some(json_lines.join("\n"))
    }
}

fn build_readable_error(text: &str, exit_code: i32) -> NormalizedText {
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        if let Some(token) = json::try_parse_token(trimmed) {
            if let Some(message) = json::try_read_string(&token, &["error", "message"]) {
                if !message.trim().is_empty() {
                    return NormalizedText::Keyed {
                        key: "Ipatool/Error/ReadableJsonError",
                        args: vec![message, exit_code.to_string()],
                    };
                }
            }
        }
        return NormalizedText::Raw(trimmed.to_string());
    }

    NormalizedText::Keyed {
        key: "Ipatool/Error/ExecutionFailed",
        args: vec![exit_code.to_string()],
    }
}

fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn email_regex() -> &'static Regex {
    static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
    EMAIL_REGEX.get_or_init(|| {
        Regex::new(r"(?i)[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}")
            .expect("email regex must compile")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_email_reads_json_aliases_and_text_fallback() {
        let cases = [
            (
                Some("{\"email\":\" user@example.com \"}"),
                "user@example.com",
            ),
            (
                Some("{\"eamil\":\"legacy@example.com\"}"),
                "legacy@example.com",
            ),
            (Some("Account: prose@example.com"), "prose@example.com"),
            (None, ""),
            (Some("no account present"), ""),
        ];

        for (payload, expected) in cases {
            assert_eq!(extract_email(payload), expected);
        }
    }

    #[test]
    fn is_payload_success_recognizes_success_formats() {
        for payload in [
            "{\"success\":true}",
            "{\"success\":\"true\"}",
            "{\"success\":1}",
            "debug\n{\"success\":true}",
            "success=true",
        ] {
            assert!(is_success(Some(payload)), "should succeed: {payload}");
        }
    }

    #[test]
    fn is_payload_success_rejects_non_success_payloads() {
        for payload in [
            None,
            Some("{\"success\":false}"),
            Some("success=false"),
            Some("no status"),
        ] {
            assert!(!is_success(payload));
        }
    }

    #[test]
    fn has_explicit_failure_flag_recognizes_failure_formats() {
        for payload in [
            "{\"success\":false}",
            "{\"success\":\"false\"}",
            "{\"success\":0}",
            "success=false",
        ] {
            assert!(
                has_explicit_failure(Some(payload)),
                "should fail: {payload}"
            );
        }
    }

    #[test]
    fn is_account_missing_from_keyring_requires_both_known_phrases() {
        for payload in [
            "failed to get account: the item could not be found in the keyring",
            "FAILED TO GET ACCOUNT; COULD NOT BE FOUND IN THE KEYRING",
        ] {
            assert!(is_account_missing_from_keyring(Some(payload)));
        }

        for payload in [
            "failed to get account",
            "could not be found in the keyring",
            "other error",
        ] {
            assert!(!is_account_missing_from_keyring(Some(payload)));
        }
    }

    #[test]
    fn normalize_streams_preserves_json_output_and_uses_stderr_when_output_is_empty() {
        let json_result = normalize_streams(Some("  {\"success\":true}  "), None, 0);
        let error_result = normalize_streams(None, Some("permission denied"), 1);

        assert_eq!(
            json_result.output,
            NormalizedText::Raw("{\"success\":true}".to_string())
        );
        assert_eq!(
            error_result.output,
            NormalizedText::Raw("permission denied".to_string())
        );
    }

    #[test]
    fn normalize_streams_extracts_json_lines_from_noisy_output() {
        let result = normalize_streams(Some("debug\n{\"success\":true}\ntrace"), None, 0);

        assert_eq!(
            result.output,
            NormalizedText::Raw("{\"success\":true}".to_string())
        );
    }

    #[test]
    fn normalize_streams_preserves_plain_text_output_without_json() {
        let result = normalize_streams(Some("plain output"), None, 0);

        assert_eq!(
            result.output,
            NormalizedText::Raw("plain output".to_string())
        );
        assert_eq!(result.error, NormalizedText::Raw(String::new()));
    }

    #[test]
    fn normalize_streams_extracts_json_lines_from_noisy_error_stream() {
        let result = normalize_streams(Some("{\"a\":1}"), Some("debug\n{\"b\":2}"), 0);

        assert_eq!(result.output, NormalizedText::Raw("{\"a\":1}".to_string()));
        assert_eq!(result.error, NormalizedText::Raw("{\"b\":2}".to_string()));
    }

    #[test]
    fn build_readable_error_returns_keyed_message_for_json_errors() {
        let result = normalize_streams(None, Some("{\"error\":\"keyring broken\"}"), 1);

        assert_eq!(
            result.output,
            NormalizedText::Keyed {
                key: "Ipatool/Error/ReadableJsonError",
                args: vec!["keyring broken".to_string(), "1".to_string()],
            }
        );
        assert_eq!(result.output.key(), "Ipatool/Error/ReadableJsonError");
    }

    #[test]
    fn build_readable_error_returns_execution_failed_key_for_empty_text() {
        let result = normalize_streams(Some("   "), None, -1);

        assert_eq!(
            result.output,
            NormalizedText::Keyed {
                key: "Ipatool/Error/ExecutionFailed",
                args: vec!["-1".to_string()],
            }
        );
    }
}
