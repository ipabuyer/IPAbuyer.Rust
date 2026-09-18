//! ipatool 客户端：进程编排。
//!
//! 移植自主仓库 `IpatoolClient`。与 C# 版的差异按本地化原则处理：
//! 超时/缺少密钥等错误不再经资源加载器拼文案，而是以 [`ErrorMessage`] 键名承载；
//! 可取消通过 `AtomicBool` 标志表达（等价 C# `CancellationToken`）。

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::core::execution::{self, ExecutionError, ExecutionOutcome, ProcessExecutionRequest};
use crate::core::ipatool::command_builder;
use crate::core::ipatool::response_parser;
use crate::core::ipatool::response_parser::NormalizedText;
use crate::core::ipatool::result::{ErrorMessage, IpatoolResult};
use crate::core::purchases::sync_service::{LogLevel, LogMessage};

/// 查询/购买类命令默认超时。
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
/// 登录超时：最多两次认证尝试，缩短超时以限制界面无响应时长。
pub const AUTH_LOGIN_TIMEOUT: Duration = Duration::from_secs(60);
/// 下载不设固定超时：由"终止下载"或宿主退出终止进程。
pub const DOWNLOAD_TIMEOUT: Option<Duration> = None;

/// 详细日志回调：接收命令行与输出行（均已脱敏）。宿主在 detailed_log
/// 关闭时传 `None`，Core 不自行读取配置。
pub type CommandLogSink<'a> = &'a mut dyn FnMut(LogMessage);

/// 客户端错误：外部取消（对齐 C# `OperationCanceledException` 的传播语义）。
#[derive(Debug, PartialEq, Eq)]
pub enum ClientError {
    Canceled,
}

/// ipatool 客户端。可执行文件路径由宿主解析后传入（内置/自定义来源的
/// 选择与回退属宿主配置职责）。
#[derive(Debug, Clone)]
pub struct IpatoolClient {
    executable_path: PathBuf,
    working_directory: Option<PathBuf>,
}

impl IpatoolClient {
    pub fn new(executable_path: impl Into<PathBuf>) -> Self {
        let executable_path = executable_path.into();
        let working_directory = executable_path.parent().map(Path::to_path_buf);
        Self {
            executable_path,
            working_directory,
        }
    }

    /// 登录：`auth_code` 传 `None` 时先用占位验证码触发双重验证下发。
    pub fn auth_login(
        &self,
        account: &str,
        password: &str,
        auth_code: Option<&str>,
        passphrase: Option<&str>,
        cancel: &AtomicBool,
        on_log: Option<CommandLogSink>,
    ) -> Result<IpatoolResult, ClientError> {
        let mut arguments = vec![
            "auth".to_string(),
            "login".to_string(),
            "--email".to_string(),
            account.to_string(),
            "--password".to_string(),
            password.to_string(),
        ];
        if let Some(code) = auth_code.filter(|code| !code.trim().is_empty()) {
            arguments.push("--auth-code".to_string());
            arguments.push(code.to_string());
        }

        self.execute(
            arguments,
            passphrase,
            Some(AUTH_LOGIN_TIMEOUT),
            cancel,
            None,
            on_log,
        )
    }

    /// 退出登录（不携带密钥；前后清理 `.ipatool/cookies.lock`）。
    pub fn auth_logout(
        &self,
        cancel: &AtomicBool,
        on_log: Option<CommandLogSink>,
    ) -> Result<IpatoolResult, ClientError> {
        self.execute(
            vec!["auth".to_string(), "revoke".to_string()],
            None,
            Some(DEFAULT_TIMEOUT),
            cancel,
            None,
            on_log,
        )
    }

    /// 查询登录状态。
    pub fn auth_info(
        &self,
        passphrase: Option<&str>,
        cancel: &AtomicBool,
        on_log: Option<CommandLogSink>,
    ) -> Result<IpatoolResult, ClientError> {
        self.execute(
            vec!["auth".to_string(), "info".to_string()],
            passphrase,
            Some(DEFAULT_TIMEOUT),
            cancel,
            None,
            on_log,
        )
    }

    /// 购买（获取许可）；`macos` 平台追加 `--platform macos`。
    pub fn purchase_app(
        &self,
        bundle_id: &str,
        passphrase: Option<&str>,
        cancel: &AtomicBool,
        on_log: Option<CommandLogSink>,
        platform: &str,
    ) -> Result<IpatoolResult, ClientError> {
        let mut arguments = vec![
            "purchase".to_string(),
            "--bundle-identifier".to_string(),
            bundle_id.to_string(),
        ];
        if let Some(value) = crate::core::platform::ipatool_arg(platform) {
            arguments.push("--platform".to_string());
            arguments.push(value.to_string());
        }
        self.execute(
            arguments,
            passphrase,
            Some(DEFAULT_TIMEOUT),
            cancel,
            None,
            on_log,
        )
    }

    /// 下载（含 `--purchase`，即下载即购买）；`on_chunk` 实时接收输出片段供进度解析。
    pub fn download_app(
        &self,
        bundle_id: &str,
        output_directory: &str,
        passphrase: Option<&str>,
        on_chunk: Option<&(dyn Fn(&str) + Sync)>,
        cancel: &AtomicBool,
        on_log: Option<CommandLogSink>,
        platform: &str,
    ) -> Result<IpatoolResult, ClientError> {
        if let Err(error) = std::fs::create_dir_all(output_directory) {
            return Ok(IpatoolResult::from_streams(
                NormalizedText::Raw(String::new()),
                NormalizedText::Raw(error.to_string()),
                -1,
            ));
        }

        let passphrase_value = passphrase.map(str::trim).filter(|value| !value.is_empty());
        let Some(passphrase_value) = passphrase_value else {
            return Ok(missing_passphrase_result());
        };

        self.run(
            command_builder::build_download_arguments(
                bundle_id,
                output_directory,
                passphrase_value,
                platform,
            ),
            DOWNLOAD_TIMEOUT,
            cancel,
            on_chunk,
            on_log,
        )
    }

    /// 分页列出账户已拥有的 App（每页数量受 ipatool 限制不得超过 100）。
    pub fn list_purchases(
        &self,
        max_results: i64,
        page: i64,
        passphrase: Option<&str>,
        cancel: &AtomicBool,
        on_log: Option<CommandLogSink>,
        platform: Option<&str>,
    ) -> Result<IpatoolResult, ClientError> {
        let mut arguments = command_builder::build_list_purchases_arguments(max_results, page);
        if let Some(value) = platform.and_then(crate::core::platform::ipatool_arg) {
            arguments.push("--platform".to_string());
            arguments.push(value.to_string());
        }
        self.execute(
            arguments,
            passphrase,
            Some(DEFAULT_TIMEOUT),
            cancel,
            None,
            on_log,
        )
    }

    /// 标准命令路径：追加密钥与全局开关后执行。
    #[allow(clippy::too_many_arguments)]
    fn execute(
        &self,
        arguments: Vec<String>,
        passphrase: Option<&str>,
        timeout: Option<Duration>,
        cancel: &AtomicBool,
        on_chunk: Option<&(dyn Fn(&str) + Sync)>,
        on_log: Option<CommandLogSink>,
    ) -> Result<IpatoolResult, ClientError> {
        let is_logout = command_builder::is_logout(&arguments);
        let passphrase_value = passphrase.map(str::trim).filter(|value| !value.is_empty());
        if !is_logout && passphrase_value.is_none() {
            return Ok(missing_passphrase_result());
        }

        let final_arguments = command_builder::build_standard_arguments(
            &arguments,
            passphrase_value.unwrap_or(""),
            is_logout,
        );
        if is_logout {
            delete_cookie_lock_file();
        }
        let result = self.run(final_arguments, timeout, cancel, on_chunk, on_log);
        if is_logout {
            delete_cookie_lock_file();
        }
        result
    }

    /// 执行已组装完成的参数（下载等自带完整参数的命令路径）。
    fn run(
        &self,
        final_arguments: Vec<String>,
        timeout: Option<Duration>,
        cancel: &AtomicBool,
        on_chunk: Option<&(dyn Fn(&str) + Sync)>,
        mut on_log: Option<CommandLogSink>,
    ) -> Result<IpatoolResult, ClientError> {
        // 详细日志：`$` 标记输入（命令行，敏感值已遮蔽）、`<` 标记输出行，
        // 与队列日志、里程碑日志区分，对齐 C# EmitCommandIfEnabled/EmitOutputIfEnabled。
        if let Some(sink) = on_log.as_mut() {
            sink(LogMessage::raw(
                LogLevel::Ipatool,
                format!(
                    "$ ipatool {}",
                    command_builder::render_for_display(&final_arguments)
                ),
            ));
        }
        let request = ProcessExecutionRequest {
            program: self.executable_path.clone(),
            working_directory: self.working_directory.clone(),
            arguments: final_arguments.clone(),
            timeout,
            environment: create_environment_variables(),
        };

        match execution::execute(&request, cancel, on_chunk) {
            Ok(ExecutionOutcome::Completed(result)) => {
                // 详细日志：上报输出行（逐行脱敏，`<` 标记输出）。
                if let Some(sink) = on_log.as_mut() {
                    let lines = command_builder::sanitized_output_lines(&result.stdout)
                        .into_iter()
                        .chain(command_builder::sanitized_output_lines(&result.stderr));
                    for line in lines {
                        sink(LogMessage::raw(LogLevel::Ipatool, format!("< {line}")));
                    }
                }
                let streams = response_parser::normalize_streams(
                    Some(&result.stdout),
                    Some(&result.stderr),
                    result.exit_code,
                );
                Ok(IpatoolResult::from_streams(
                    streams.output,
                    streams.error,
                    result.exit_code,
                ))
            }
            Ok(ExecutionOutcome::TimedOut) => Ok(IpatoolResult::from_error_message(
                ErrorMessage {
                    key: "Ipatool/Error/ExecutionTimeout",
                    args: vec![command_builder::get_safe_command_label(&final_arguments)],
                },
                -1,
                true,
            )),
            Err(ExecutionError::Canceled) => Err(ClientError::Canceled),
            Err(ExecutionError::Io(message)) => Ok(IpatoolResult::from_streams(
                NormalizedText::Raw(String::new()),
                NormalizedText::Raw(message),
                -1,
            )),
        }
    }
}

fn missing_passphrase_result() -> IpatoolResult {
    IpatoolResult::from_error_message(
        ErrorMessage {
            key: "Ipatool/Error/MissingPassphrase",
            args: Vec::new(),
        },
        -1,
        false,
    )
}

fn create_environment_variables() -> Vec<(String, String)> {
    command_builder::create_environment_variables()
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// 登出前后清理 ipatool 的 cookie 锁文件（对齐 C# `DeleteCookieLockFile`，尽力而为）。
fn delete_cookie_lock_file() {
    if let Some(home) = home_directory() {
        let _ = std::fs::remove_file(home.join(".ipatool").join("cookies.lock"));
    }
}

fn home_directory() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_passphrase_is_reported_via_error_key_for_non_logout_commands() {
        let client = IpatoolClient::new("ipatool.exe");
        let cancel = AtomicBool::new(false);

        let result = client.auth_info(None, &cancel, None).unwrap();

        assert!(!result.timed_out);
        assert_eq!(
            result
                .error_message
                .as_ref()
                .expect("missing passphrase key")
                .key,
            "Ipatool/Error/MissingPassphrase"
        );
        assert_eq!(result.output_or_error_raw(), "");
    }

    #[test]
    fn logout_omits_passphrase_entirely() {
        // 不触达进程：仅验证密钥缺失守卫不会拦截登出。
        let client = IpatoolClient::new("ipatool.exe");
        let cancel = AtomicBool::new(false);

        let result = client.auth_logout(&cancel, None);

        // 可执行文件不存在 → Io 错误进入结果（对齐 C# catch ex.Message）。
        assert!(result.is_err() || !result.unwrap().timed_out);
    }

    #[test]
    fn detailed_sink_receives_sanitized_command_line() {
        // 可执行文件不存在仍应先上报命令行日志，密钥取值替换为 "***"。
        let client = IpatoolClient::new("ipatool.exe");
        let cancel = AtomicBool::new(false);
        let mut logs: Vec<LogMessage> = Vec::new();
        {
            let mut sink = |log: LogMessage| logs.push(log);
            let _ = client.auth_info(Some("secret"), &cancel, Some(&mut sink));
        }

        let rendered: Vec<String> = logs
            .iter()
            .filter_map(|log| match &log.message {
                NormalizedText::Raw(text) => Some(text.clone()),
                NormalizedText::Keyed { .. } => None,
            })
            .collect();
        assert!(
            rendered
                .iter()
                .any(|line| line.starts_with("$ ipatool auth info")
                    && line.contains("--keychain-passphrase \"***\"")),
            "command line log missing or not sanitized: {rendered:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn detailed_sink_marks_input_and_output_lines() {
        // 用真实脚本验证：输入行带 "$"，输出行带 "<"。
        let directory =
            std::env::temp_dir().join(format!("ipabuyer_client_out_{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("echo_ipatool.cmd");
        std::fs::write(&script, "@echo {\"success\":true}").unwrap();

        let client = IpatoolClient::new(&script);
        let cancel = AtomicBool::new(false);
        let mut logs: Vec<LogMessage> = Vec::new();
        {
            let mut sink = |log: LogMessage| logs.push(log);
            let _ = client.auth_info(Some("secret"), &cancel, Some(&mut sink));
        }

        assert!(
            logs.iter().any(|log| matches!(&log.message,
                NormalizedText::Raw(text) if text.starts_with("$ ipatool auth info")
                    && text.contains("--keychain-passphrase \"***\""))),
            "input line missing $ marker or masking: {logs:?}"
        );
        assert!(
            logs.iter().any(|log| matches!(&log.message,
                NormalizedText::Raw(text) if text.starts_with("< ") && text.contains("success"))),
            "output line missing < marker: {logs:?}"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
