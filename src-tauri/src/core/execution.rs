//! 子进程执行基础设施（移植自主仓库 `IPAbuyer.Core.Execution`）。
//!
//! 行为对齐：标准输入启动后立即关闭、UTF-8 流收集、超时/取消时终止整棵进程树
//! （Windows 使用 `taskkill /T /F`，等价 C# `Kill(entireProcessTree: true)`）、
//! 取消与超时分别上报。

use std::io::{BufReader, Read};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// 轮询进程退出的间隔。
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// 单次进程执行请求。
#[derive(Debug, Clone)]
pub struct ProcessExecutionRequest {
    pub program: PathBuf,
    pub working_directory: Option<PathBuf>,
    pub arguments: Vec<String>,
    /// `None` 表示不设超时（如下载）。
    pub timeout: Option<Duration>,
    pub environment: Vec<(String, String)>,
}

/// 进程正常退出后的流与退出码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// 执行结果：正常完成或超时（取消经 [`ExecutionError::Canceled`] 上报）。
#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Completed(ProcessExecutionResult),
    TimedOut,
}

/// 执行错误：外部取消或进程启动失败。
#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionError {
    Canceled,
    Io(String),
}

/// 执行子进程并收集输出。`on_output_chunk` 在读取线程上被调用（需 `Sync`）。
pub fn execute(
    request: &ProcessExecutionRequest,
    cancel: &AtomicBool,
    on_output_chunk: Option<&(dyn Fn(&str) + Sync)>,
) -> Result<ExecutionOutcome, ExecutionError> {
    let mut command = Command::new(&request.program);
    command.args(&request.arguments);
    if let Some(directory) = &request.working_directory {
        command.current_dir(directory);
    }
    for (key, value) in &request.environment {
        command.env(key, value);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|error| ExecutionError::Io(error.to_string()))?;
    // 立即关闭标准输入：任何交互提示都会因 EOF 立即结束，而不是挂起等待。
    drop(child.stdin.take());

    let stdout = child.stdout.take().expect("stdout must be piped");
    let stderr = child.stderr.take().expect("stderr must be piped");
    let stdout_buffer = Arc::new(Mutex::new(Vec::new()));
    let stderr_buffer = Arc::new(Mutex::new(Vec::new()));
    let started = Instant::now();

    let mut outcome: Result<ExecutionOutcome, ExecutionError> = Err(ExecutionError::Io(
        "process loop ended unexpectedly".to_string(),
    ));

    thread::scope(|scope| {
        scope.spawn(|| read_stream(stdout, stdout_buffer.clone(), on_output_chunk));
        scope.spawn(|| read_stream(stderr, stderr_buffer.clone(), on_output_chunk));

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let result = ProcessExecutionResult {
                        stdout: decode_buffer(&stdout_buffer),
                        stderr: decode_buffer(&stderr_buffer),
                        exit_code: status.code().unwrap_or(-1),
                    };
                    outcome = Ok(ExecutionOutcome::Completed(result));
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    outcome = Err(ExecutionError::Io(error.to_string()));
                    break;
                }
            }

            if cancel.load(Ordering::Relaxed) {
                terminate(&mut child);
                outcome = Err(ExecutionError::Canceled);
                break;
            }

            if let Some(timeout) = request.timeout {
                if started.elapsed() >= timeout {
                    terminate(&mut child);
                    outcome = Ok(ExecutionOutcome::TimedOut);
                    break;
                }
            }

            thread::sleep(POLL_INTERVAL);
        }
    });

    outcome
}

fn read_stream<R: Read>(
    stream: R,
    buffer: Arc<Mutex<Vec<u8>>>,
    on_chunk: Option<&(dyn Fn(&str) + Sync)>,
) {
    let mut reader = BufReader::new(stream);
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                buffer
                    .lock()
                    .expect("stream buffer lock")
                    .extend_from_slice(&chunk[..read]);
                if let Some(callback) = on_chunk {
                    callback(&String::from_utf8_lossy(&chunk[..read]));
                }
            }
            Err(_) => break,
        }
    }
}

/// 终止整棵进程树并回收子进程。
fn terminate(child: &mut Child) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if let Some(pid) = child.id().checked_sub(0).map(|id| id.to_string()) {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid, "/T", "/F"])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn decode_buffer(buffer: &Arc<Mutex<Vec<u8>>>) -> String {
    let bytes = buffer.lock().expect("stream buffer lock");
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn execute_runs_script_and_collects_streams() {
        let directory =
            std::env::temp_dir().join(format!("ipabuyer_core_exec_{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("echo_args.cmd");
        std::fs::write(&script, "@echo {\"success\":true}").unwrap();

        let cancel = AtomicBool::new(false);
        let request = ProcessExecutionRequest {
            program: script.clone(),
            working_directory: Some(directory.clone()),
            arguments: Vec::new(),
            timeout: Some(Duration::from_secs(30)),
            environment: Vec::new(),
        };

        let outcome = execute(&request, &cancel, None).unwrap();

        match outcome {
            ExecutionOutcome::Completed(result) => {
                assert_eq!(result.exit_code, 0);
                assert_eq!(result.stdout.trim(), "{\"success\":true}");
            }
            other => panic!("expected completion, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[cfg(windows)]
    #[test]
    fn execute_times_out_and_terminates_process_tree() {
        let directory =
            std::env::temp_dir().join(format!("ipabuyer_core_exec_to_{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("sleep.cmd");
        std::fs::write(&script, "@echo off\r\nping -n 30 127.0.0.1 > nul\r\n").unwrap();

        let cancel = AtomicBool::new(false);
        let request = ProcessExecutionRequest {
            program: script.clone(),
            working_directory: Some(directory.clone()),
            arguments: Vec::new(),
            timeout: Some(Duration::from_millis(300)),
            environment: Vec::new(),
        };

        let started = Instant::now();
        let outcome = execute(&request, &cancel, None).unwrap();

        assert_eq!(outcome, ExecutionOutcome::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "tree kill must not wait for the sleeping child"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
