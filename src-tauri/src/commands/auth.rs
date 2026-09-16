//! 认证命令：登录 / 双重验证 / 登出 / 查询登录状态。
//!
//! 对齐 C# LoginService / IpatoolClient.AuthInfoAsync / LoginPage 的编排：
//! 密钥解析（显式 > 已存 > 新生成，登录成功后落库）、模拟账户识别、
//! 登出后按开关轮换密钥。core 调用为阻塞式（ipatool 子进程，最长 2 分钟），
//! 命令标记 `(async)` 在独立线程执行——Tauri 同步命令默认运行在主线程，
//! 会阻塞事件循环冻结 UI/光标。

use std::sync::atomic::AtomicBool;

use serde::Serialize;
use tauri::State;

use crate::commands::{JsMessage, LogEntryDto};
use crate::core::auth::login::{self, LoginResult, LoginStatus};
use crate::core::ipatool::client::{ClientError, IpatoolClient};
use crate::core::ipatool::response_parser::{
    extract_email, has_explicit_failure, is_account_missing_from_keyring, is_success,
};
use crate::state::{self, AppState};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResultDto {
    pub status: String,
    pub message: JsMessage,
    pub raw_payload: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthInfoDto {
    /// LoggedIn / NotLoggedIn / Error
    pub status: String,
    pub email: Option<String>,
    pub message: Option<JsMessage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoutDto {
    pub success: bool,
    /// 登出成功且密钥轮换开关打开时为 true（新密钥已写入凭据管理器）。
    pub passphrase_rotated: bool,
    pub message: Option<JsMessage>,
}

fn status_name(status: LoginStatus) -> String {
    match status {
        LoginStatus::Success => "Success",
        LoginStatus::RequiresTwoFactor => "RequiresTwoFactor",
        LoginStatus::InvalidCredential => "InvalidCredential",
        LoginStatus::AuthCodeInvalid => "AuthCodeInvalid",
        LoginStatus::NetworkError => "NetworkError",
        LoginStatus::Timeout => "Timeout",
        LoginStatus::UnknownError => "UnknownError",
    }
    .into()
}

fn to_dto(result: LoginResult) -> AuthResultDto {
    AuthResultDto {
        status: status_name(result.status),
        message: (&result.message).into(),
        raw_payload: result.raw_payload,
    }
}

fn client_error_message(error: ClientError) -> String {
    match error {
        ClientError::Canceled => "操作已取消".into(),
    }
}

/// 认证动作写日志缓冲（与 sync/queue/purchases 同路径，经轮询 emit）。
fn push_log(state: &AppState, level: &str, key: &str, args: &[&str]) {
    let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    state
        .log_buffer
        .push(LogEntryDto::new(level, JsMessage::key(key, &owned)));
}

fn execute_login(
    state: &AppState,
    account: String,
    password: String,
    passphrase_input: Option<String>,
    auth_code: Option<String>,
) -> Result<AuthResultDto, String> {
    let account = account.trim().to_string();
    let password = password.trim().to_string();
    if account.is_empty() || password.is_empty() {
        return Err("请填写 Apple 账户与密码".into());
    }

    let exe_path = crate::resolver::resolve_executable_path(state);
    let (passphrase, generated) = state::resolve_passphrase(passphrase_input.as_deref());
    let explicit = passphrase_input
        .as_deref()
        .is_some_and(|p| !p.trim().is_empty());

    let client = IpatoolClient::new(exe_path);
    let cancel = AtomicBool::new(false);
    let result = match auth_code.as_deref() {
        Some(code) => {
            push_log(state, "info", "Auth/Log/VerifyStart", &[&account]);
            login::verify_auth_code(
                &client,
                &account,
                &password,
                Some(&passphrase),
                code,
                &cancel,
                None,
            )
        }
        None => {
            push_log(state, "info", "Auth/Log/LoginStart", &[&account]);
            login::login(&client, &account, &password, Some(&passphrase), &cancel, None)
        }
    };

    if result.is_success() {
        // 登录成功才持久化密钥（新生成或显式传入的都存，对齐 C# OnLoginSuccessAsync）
        if generated || explicit {
            state::save_passphrase(&passphrase)?;
        }
        let is_mock = login::is_mock_account(Some(&account), Some(&password));
        state.session.lock().unwrap().set_login(account.clone(), is_mock);
        push_log(state, "success", "Auth/Log/LoginSuccess", &[&account]);
    } else if result.status == LoginStatus::RequiresTwoFactor {
        push_log(state, "tip", "Auth/Log/TwoFactorRequired", &[] as &[&str]);
    } else {
        push_log(state, "error", "Auth/Log/LoginFailed", &[]);
    }

    Ok(to_dto(result))
}

#[tauri::command(async)]
pub fn auth_login(
    state: State<'_, AppState>,
    account: String,
    password: String,
    passphrase: Option<String>,
) -> Result<AuthResultDto, String> {
    execute_login(&state, account, password, passphrase, None)
}

#[tauri::command(async)]
pub fn auth_verify_code(
    state: State<'_, AppState>,
    account: String,
    password: String,
    auth_code: String,
    passphrase: Option<String>,
) -> Result<AuthResultDto, String> {
    execute_login(&state, account, password, passphrase, Some(auth_code))
}

#[tauri::command(async)]
pub fn auth_logout(state: State<'_, AppState>) -> Result<LogoutDto, String> {
    push_log(&state, "info", "Auth/Log/LogoutStart", &[]);
    let exe_path = crate::resolver::resolve_executable_path(&state);
    let client = IpatoolClient::new(exe_path);
    let cancel = AtomicBool::new(false);
    let result = client
        .auth_logout(&cancel, None)
        .map_err(|e| client_error_message(e))?;

    if result.timed_out || result.error_message.is_some() {
        push_log(&state, "error", "Auth/Log/LogoutFailed", &[]);
        return Ok(LogoutDto {
            success: false,
            passphrase_rotated: false,
            message: Some((&result.error).into()),
        });
    }

    let rotation_enabled = state.config.lock().unwrap().passphrase_rotation_enabled;
    let mut rotated = false;
    if rotation_enabled {
        state::save_passphrase(&state::generate_passphrase())?;
        rotated = true;
    }
    state.session.lock().unwrap().reset();

    if rotated {
        push_log(&state, "success", "Auth/Log/LogoutSuccessRotated", &[]);
    } else {
        push_log(&state, "success", "Auth/Log/LogoutSuccess", &[]);
    }

    Ok(LogoutDto {
        success: true,
        passphrase_rotated: rotated,
        message: None,
    })
}

/// 查询登录状态（对应启动时静默 Warmup 与账户页"查询登录状态"按钮）。
/// 查询使用已存密钥；ipatool keyring 无账号时视为未登录。
#[tauri::command(async)]
pub fn auth_info(state: State<'_, AppState>) -> Result<AuthInfoDto, String> {
    let exe_path = crate::resolver::resolve_executable_path(&state);
    let passphrase = state::get_passphrase();

    let client = IpatoolClient::new(exe_path);
    let cancel = AtomicBool::new(false);
    let result = client
        .auth_info(passphrase.as_deref(), &cancel, None)
        .map_err(|e| client_error_message(e))?;

    let payload = result.output.as_raw().to_string();

    if is_account_missing_from_keyring(Some(&payload)) {
        state.session.lock().unwrap().reset();
        push_log(&state, "info", "Auth/Log/QueryNotLoggedIn", &[] as &[&str]);
        return Ok(AuthInfoDto {
            status: "NotLoggedIn".into(),
            email: None,
            message: Some(JsMessage::key("LoginPage/Status/AuthInfoNotLoggedIn", &[])),
        });
    }

    let email = extract_email(Some(&payload));
    let command_ok = !result.timed_out && result.error_message.is_none();
    let auth_ok = command_ok
        && is_success(Some(&payload))
        && !has_explicit_failure(Some(&payload))
        && (is_success(Some(&payload)) || !email.is_empty());

    if auth_ok {
        if !email.is_empty() {
            let is_mock = state.session.lock().unwrap().is_mock;
            state.session.lock().unwrap().set_login(email.clone(), is_mock);
            push_log(&state, "success", "Auth/Log/QueryLoggedIn", &[email.as_str()]);
        }
        Ok(AuthInfoDto {
            status: "LoggedIn".into(),
            email: Some(email),
            message: None,
        })
    } else {
        push_log(&state, "error", "Auth/Log/QueryFailed", &[] as &[&str]);
        let message = match &result.error_message {
            Some(error) => JsMessage::key(error.key, &error.args),
            None => JsMessage::raw(result.error.as_raw().to_string()),
        };
        Ok(AuthInfoDto {
            status: "Error".into(),
            email: None,
            message: Some(message),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::auth::login::Message;

    fn result_of(status: LoginStatus) -> LoginResult {
        LoginResult {
            status,
            message: Message::Key {
                key: "Some/Key",
                args: Vec::new(),
            },
            raw_payload: Some("raw".into()),
        }
    }

    #[test]
    fn status_name_covers_all_statuses() {
        assert_eq!(status_name(LoginStatus::Success), "Success");
        assert_eq!(status_name(LoginStatus::RequiresTwoFactor), "RequiresTwoFactor");
        assert_eq!(status_name(LoginStatus::InvalidCredential), "InvalidCredential");
        assert_eq!(status_name(LoginStatus::AuthCodeInvalid), "AuthCodeInvalid");
        assert_eq!(status_name(LoginStatus::NetworkError), "NetworkError");
        assert_eq!(status_name(LoginStatus::Timeout), "Timeout");
        assert_eq!(status_name(LoginStatus::UnknownError), "UnknownError");
    }

    #[test]
    fn to_dto_maps_message_and_payload() {
        let dto = to_dto(result_of(LoginStatus::Success));
        assert_eq!(dto.status, "Success");
        assert_eq!(dto.raw_payload.as_deref(), Some("raw"));
        assert!(matches!(dto.message, JsMessage::Key { key, .. } if key == "Some/Key"));
    }

    #[test]
    fn client_error_message_localizes_cancel() {
        assert_eq!(client_error_message(ClientError::Canceled), "操作已取消");
    }
}
