//! 登录流程与结果分类。
//!
//! 移植自主仓库 `LoginService`。本地化差异：结果消息按本地化原则以键名 +
//! 参数（[`Message`]）承载，宿主用 `.resw` 渲染；无法识别的错误原文以
//! [`Message::Raw`] 透传。

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::core::ipatool::client::{ClientError, CommandLogSink, IpatoolClient};
use crate::core::json;

/// 模拟账户用户名（测试用途：购买/下载一律直接成功）。
const MOCK_USERNAME: &str = "test";
/// 模拟账户密码。
const MOCK_PASSWORD: &str = "test";
/// 模拟登录的延迟，模拟真实认证耗时。
const MOCK_LOGIN_DELAY: Duration = Duration::from_secs(1);

/// 登录状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginStatus {
    Success,
    RequiresTwoFactor,
    InvalidCredential,
    AuthCodeInvalid,
    NetworkError,
    Timeout,
    UnknownError,
}

/// 待宿主本地化的消息：键名 + 参数，或原文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Key {
        key: &'static str,
        args: Vec<String>,
    },
    Raw(String),
}

impl Message {
    fn key(key: &'static str) -> Self {
        Message::Key {
            key,
            args: Vec::new(),
        }
    }
}

/// 登录结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginResult {
    pub status: LoginStatus,
    pub message: Message,
    pub raw_payload: Option<String>,
}

impl LoginResult {
    pub fn is_success(&self) -> bool {
        self.status == LoginStatus::Success
    }

    pub fn requires_two_factor(&self) -> bool {
        self.status == LoginStatus::RequiresTwoFactor
    }
}

/// 判断是否为模拟账户（大小写不敏感用户名 + 大小写敏感密码，对齐 C#）。
pub fn is_mock_account(username: Option<&str>, password: Option<&str>) -> bool {
    let (Some(username), Some(password)) = (username, password) else {
        return false;
    };
    let username = username.trim();
    let password = password.trim();
    !username.is_empty()
        && !password.is_empty()
        && username.eq_ignore_ascii_case(MOCK_USERNAME)
        && password == MOCK_PASSWORD
}

/// 登录：先用占位验证码 `000000` 触发双重验证码下发。
pub fn login(
    client: &IpatoolClient,
    account: &str,
    password: &str,
    passphrase: Option<&str>,
    cancel: &AtomicBool,
    on_log: Option<CommandLogSink>,
) -> LoginResult {
    execute_login(
        client, account, password, passphrase, "000000", false, cancel, on_log,
    )
}

/// 双重验证阶段：携带真实验证码完成登录。
pub fn verify_auth_code(
    client: &IpatoolClient,
    account: &str,
    password: &str,
    passphrase: Option<&str>,
    auth_code: &str,
    cancel: &AtomicBool,
    on_log: Option<CommandLogSink>,
) -> LoginResult {
    execute_login(
        client, account, password, passphrase, auth_code, true, cancel, on_log,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_login(
    client: &IpatoolClient,
    account: &str,
    password: &str,
    passphrase: Option<&str>,
    auth_code: &str,
    is_two_factor: bool,
    cancel: &AtomicBool,
    on_log: Option<&mut dyn FnMut(crate::core::purchases::sync_service::LogMessage)>,
) -> LoginResult {
    if is_mock_account(Some(account), Some(password)) {
        if !sleep_cancellable(MOCK_LOGIN_DELAY, cancel) {
            return LoginResult {
                status: LoginStatus::UnknownError,
                message: Message::key("LoginService/Status/Canceled"),
                raw_payload: None,
            };
        }
        return LoginResult {
            status: LoginStatus::Success,
            message: Message::key("LoginService/Status/Success"),
            raw_payload: None,
        };
    }

    let response = client.auth_login(
        account,
        password,
        Some(auth_code),
        passphrase,
        cancel,
        on_log,
    );
    match response {
        Err(ClientError::Canceled) => LoginResult {
            status: LoginStatus::UnknownError,
            message: Message::key("LoginService/Status/Canceled"),
            raw_payload: None,
        },
        Ok(result) if result.timed_out => LoginResult {
            status: LoginStatus::Timeout,
            message: Message::key("LoginService/Status/Timeout"),
            raw_payload: None,
        },
        Ok(result) => {
            let payload = result.output_or_error_raw();
            if payload.trim().is_empty() {
                LoginResult {
                    status: LoginStatus::UnknownError,
                    message: Message::key("LoginService/Status/EmptyResponse"),
                    raw_payload: None,
                }
            } else {
                interpret_payload(&payload, is_two_factor)
            }
        }
    }
}

fn interpret_payload(payload: &str, is_two_factor: bool) -> LoginResult {
    for token in json::enumerate_tokens(Some(payload)) {
        if let Some(success) = json::try_read_boolean(&token, "success") {
            if success {
                return LoginResult {
                    status: LoginStatus::Success,
                    message: Message::key("LoginService/Status/Success"),
                    raw_payload: Some(payload.to_string()),
                };
            }

            let error =
                json::try_read_string(&token, &["error", "message", "reason"]).unwrap_or_default();
            return classify_failure(&error, payload, is_two_factor);
        }

        let message =
            json::try_read_string(&token, &["error", "message", "reason"]).unwrap_or_default();
        if !message.trim().is_empty() {
            let failure = classify_failure(&message, payload, is_two_factor);
            if failure.status != LoginStatus::UnknownError {
                return failure;
            }
        }

        // 双重验证关键词可能出现在无法结构化解析的片段里，退化为对原文匹配。
        if detect_two_factor_requirement(&token.to_string()) {
            return LoginResult {
                status: LoginStatus::RequiresTwoFactor,
                message: Message::key("LoginService/Status/RequiresTwoFactor"),
                raw_payload: Some(payload.to_string()),
            };
        }
    }

    if detect_two_factor_requirement(payload) {
        return LoginResult {
            status: LoginStatus::RequiresTwoFactor,
            message: Message::key("LoginService/Status/RequiresTwoFactor"),
            raw_payload: Some(payload.to_string()),
        };
    }

    classify_failure(payload, payload, is_two_factor)
}

/// 失败分类：顺序与 C# `ClassifyFailure` 一致。
fn classify_failure(message: &str, payload: &str, is_two_factor: bool) -> LoginResult {
    let effective = if message.trim().is_empty() {
        payload
    } else {
        message
    };

    // 双重验证阶段：错误消息（如 "invalid auth code"）会同时命中双重验证关键词，需先按验证码错误分类。
    if is_two_factor && detect_auth_code_invalid(effective) {
        return LoginResult {
            status: LoginStatus::AuthCodeInvalid,
            message: Message::key("LoginService/Status/AuthCodeInvalid"),
            raw_payload: Some(payload.to_string()),
        };
    }

    if detect_two_factor_requirement(effective)
        || (!is_two_factor && detect_generic_apple_auth_failure(effective))
    {
        return LoginResult {
            status: LoginStatus::RequiresTwoFactor,
            message: Message::key("LoginService/Status/RequiresTwoFactor"),
            raw_payload: Some(payload.to_string()),
        };
    }

    if detect_invalid_credential(effective) {
        return LoginResult {
            status: LoginStatus::InvalidCredential,
            message: Message::key("LoginService/Status/InvalidCredential"),
            raw_payload: Some(payload.to_string()),
        };
    }

    if detect_network_issue(effective) {
        return LoginResult {
            status: LoginStatus::NetworkError,
            message: Message::key("LoginService/Status/NetworkError"),
            raw_payload: Some(payload.to_string()),
        };
    }

    LoginResult {
        status: LoginStatus::UnknownError,
        message: if effective.trim().is_empty() {
            Message::key("LoginService/Status/FailedRetry")
        } else {
            Message::Raw(effective.to_string())
        },
        raw_payload: Some(payload.to_string()),
    }
}

fn detect_two_factor_requirement(message: &str) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    let lowered = message.to_lowercase();
    lowered.contains("auth code")
        || lowered.contains("two factor")
        || lowered.contains("2fa")
        || lowered.contains("请输入验证码")
        || lowered.contains("authentication code")
}

fn detect_generic_apple_auth_failure(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("something went wrong")
}

fn detect_invalid_credential(message: &str) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    let lowered = message.to_lowercase();
    lowered.contains("invalid credentials")
        || lowered.contains("incorrect")
        || lowered.contains("username or password")
        || lowered.contains("bad credentials")
}

fn detect_auth_code_invalid(message: &str) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    let lowered = message.to_lowercase();
    lowered.contains("invalid auth code")
        || lowered.contains("auth code is incorrect")
        || lowered.contains("验证码错误")
}

fn detect_network_issue(message: &str) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    let lowered = message.to_lowercase();
    lowered.contains("network")
        || lowered.contains("timeout")
        || lowered.contains("timed out")
        || lowered.contains("connection")
        || lowered.contains("ssl")
}

/// 可取消睡眠：返回 false 表示在等待期间被取消。
fn sleep_cancellable(total: Duration, cancel: &AtomicBool) -> bool {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(
            Duration::from_millis(50)
                .min(deadline.saturating_duration_since(Instant::now()))
                .max(Duration::ZERO),
        );
    }
    !cancel.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_account_requires_exact_test_credentials() {
        assert!(is_mock_account(Some("test"), Some("test")));
        assert!(is_mock_account(Some(" TEST "), Some(" test ")));
        assert!(!is_mock_account(Some("test"), Some("Test")));
        assert!(!is_mock_account(Some("tests"), Some("test")));
        assert!(!is_mock_account(Some(""), Some("test")));
        assert!(!is_mock_account(None, None));
    }

    #[test]
    fn classify_failure_detects_two_factor_requirement() {
        let result = classify_failure("please enter your auth code", "payload", false);
        assert_eq!(result.status, LoginStatus::RequiresTwoFactor);

        let result = classify_failure("请输入验证码", "payload", false);
        assert_eq!(result.status, LoginStatus::RequiresTwoFactor);
    }

    #[test]
    fn classify_failure_detects_auth_code_invalid_during_two_factor_first() {
        // "invalid auth code" 同时命中验证码错误与双重验证关键词，双重验证阶段必须先按验证码错误分类。
        let result = classify_failure("invalid auth code", "payload", true);
        assert_eq!(result.status, LoginStatus::AuthCodeInvalid);

        let result = classify_failure("验证码错误", "payload", true);
        assert_eq!(result.status, LoginStatus::AuthCodeInvalid);
    }

    #[test]
    fn classify_failure_detects_invalid_credentials() {
        let result = classify_failure("invalid credentials", "payload", false);
        assert_eq!(result.status, LoginStatus::InvalidCredential);

        let result = classify_failure("Your username or password is incorrect.", "payload", false);
        assert_eq!(result.status, LoginStatus::InvalidCredential);
    }

    #[test]
    fn classify_failure_detects_network_issues() {
        let result = classify_failure("The connection timed out", "payload", false);
        assert_eq!(result.status, LoginStatus::NetworkError);
    }

    #[test]
    fn classify_failure_detects_generic_apple_auth_failure_only_outside_two_factor() {
        let outside = classify_failure("Something went wrong", "payload", false);
        assert_eq!(outside.status, LoginStatus::RequiresTwoFactor);

        let inside = classify_failure("Something went wrong", "payload", true);
        assert_eq!(inside.status, LoginStatus::UnknownError);
    }

    #[test]
    fn classify_failure_unknown_keeps_raw_message_or_retry_key() {
        let raw = classify_failure("mystery", "payload", false);
        assert_eq!(raw.status, LoginStatus::UnknownError);
        assert_eq!(raw.message, Message::Raw("mystery".to_string()));

        // C# 语义：message 为空时回退到 payload 原文，两者皆空才用 FailedRetry 键。
        let fallback_payload = classify_failure("", "payload", false);
        assert_eq!(
            fallback_payload.message,
            Message::Raw("payload".to_string())
        );

        let retry = classify_failure("", "", false);
        assert_eq!(
            retry.message,
            Message::key("LoginService/Status/FailedRetry")
        );
    }

    #[test]
    fn interpret_payload_reads_success_token() {
        let result = interpret_payload("{\"success\":true}", false);
        assert_eq!(result.status, LoginStatus::Success);
        assert_eq!(result.raw_payload.as_deref(), Some("{\"success\":true}"));
    }

    #[test]
    fn interpret_payload_reads_error_field_and_classifies() {
        let result = interpret_payload(
            "{\"success\":false,\"error\":\"invalid credentials\"}",
            false,
        );
        assert_eq!(result.status, LoginStatus::InvalidCredential);
    }

    #[test]
    fn interpret_payload_detects_two_factor_from_raw_text() {
        let result = interpret_payload("debug\n{\"hint\":\"2fa required\"}", false);
        assert_eq!(result.status, LoginStatus::RequiresTwoFactor);
    }
}
