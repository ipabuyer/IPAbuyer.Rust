//! 下载结果解析（移植自主仓库 `DownloadResultParser`）。
//!
//! 本地化差异：超时/退出码等错误按本地化原则返回键名（[`NormalizedText::Keyed`]）。

use crate::core::ipatool::response_parser::NormalizedText;
use crate::core::ipatool::result::IpatoolResult;
use crate::core::json;

/// 判断下载是否成功：优先取最后一个 `success[:=]` 标志，其次退出码，最后 JSON `success`。
pub fn is_success(result: &IpatoolResult) -> bool {
    let payload = result.output_or_error_raw();

    if let Some(success) = try_extract_success_flag(&payload) {
        return success;
    }

    if result.is_success_response() {
        return true;
    }

    payload.to_ascii_lowercase().contains("\"success\":true")
        || json::enumerate_tokens(Some(&payload))
            .iter()
            .any(|token| json::try_read_boolean(token, "success") == Some(true))
}

/// 提取错误消息：超时/空输出返回键名，JSON 错误字段次之，最后截断原文。
pub fn get_error_message(result: &IpatoolResult) -> NormalizedText {
    if result.timed_out {
        return NormalizedText::Keyed {
            key: "DownloadQueue/Error/Timeout",
            args: Vec::new(),
        };
    }

    let payload = result.output_or_error_raw();
    if payload.trim().is_empty() {
        return NormalizedText::Keyed {
            key: "DownloadQueue/Error/ExitCode",
            args: vec![result.exit_code.to_string()],
        };
    }

    for token in json::enumerate_tokens(Some(&payload)) {
        if let Some(detail) = json::try_read_string(&token, &["error", "message"]) {
            if !detail.trim().is_empty() {
                return NormalizedText::Raw(detail);
            }
        }
    }

    truncate(payload)
}

fn truncate(payload: String) -> NormalizedText {
    let char_count = payload.chars().count();
    if char_count > 160 {
        let truncated: String = payload.chars().take(160).collect();
        NormalizedText::Raw(format!("{truncated}..."))
    } else {
        NormalizedText::Raw(payload)
    }
}

fn try_extract_success_flag(payload: &str) -> Option<bool> {
    if payload.trim().is_empty() {
        return None;
    }

    static SUCCESS_FLAG_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let regex = SUCCESS_FLAG_REGEX.get_or_init(|| {
        regex::Regex::new(r"(?i)success\s*[:=]\s*(true|false)").expect("success regex must compile")
    });

    let last = regex.captures_iter(payload).last()?;
    Some(
        last.get(1)
            .expect("capture group")
            .as_str()
            .eq_ignore_ascii_case("true"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ipatool::response_parser::NormalizedText;

    fn result(output: &str, exit_code: i32, timed_out: bool) -> IpatoolResult {
        IpatoolResult::from_streams(
            NormalizedText::Raw(output.to_string()),
            NormalizedText::Raw(String::new()),
            exit_code,
        )
        .with_timed_out(timed_out)
    }

    #[test]
    fn is_success_prefers_last_explicit_flag() {
        assert!(is_success(&result(
            "progress... success=false then success=true",
            0,
            false
        )));
        assert!(!is_success(&result(
            "success=true then success=false",
            0,
            false
        )));
    }

    #[test]
    fn is_success_falls_back_to_exit_code_then_json() {
        assert!(is_success(&result("plain output", 0, false)));
        assert!(!is_success(&result("plain output", 1, false)));
        assert!(is_success(&result("debug\n{\"success\":true}", 1, false)));
    }

    #[test]
    fn get_error_message_reports_timeout_by_key() {
        let timed_out = IpatoolResult::from_error_message(
            crate::core::ipatool::result::ErrorMessage {
                key: "Ipatool/Error/ExecutionTimeout",
                args: vec!["ipatool download".to_string()],
            },
            -1,
            true,
        );

        assert_eq!(
            get_error_message(&timed_out),
            NormalizedText::Keyed {
                key: "DownloadQueue/Error/Timeout",
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn get_error_message_reports_blank_payload_by_exit_code() {
        let message = get_error_message(&result("", 3, false));

        assert_eq!(
            message,
            NormalizedText::Keyed {
                key: "DownloadQueue/Error/ExitCode",
                args: vec!["3".to_string()],
            }
        );
    }

    #[test]
    fn get_error_message_reads_json_error_field() {
        let message = get_error_message(&result(
            r#"{"success":false,"error":"app not found"}"#,
            1,
            false,
        ));

        assert_eq!(message, NormalizedText::Raw("app not found".to_string()));
    }

    #[test]
    fn get_error_message_truncates_long_payloads() {
        let long = "x".repeat(300);
        let message = get_error_message(&result(&long, 1, false));

        match message {
            NormalizedText::Raw(text) => {
                assert_eq!(text.chars().count(), 163);
                assert!(text.ends_with("..."));
            }
            other => panic!("expected raw message, got {other:?}"),
        }
    }
}
