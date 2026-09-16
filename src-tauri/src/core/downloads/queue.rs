//! 下载队列服务。
//!
//! 移植自主仓库 `DownloadQueueService` 的状态机：串行处理
//! Pending/Failed/Canceled 条目、单飞行守卫、mock 账户直通、逐项输出解析。
//! 与 C# 版的差异：条目集合为 `Mutex<Vec<_>>`（ObservableCollection 属宿主
//! UI）；`last_message` 按本地化原则存储 resw 键名；取消经单一 `AtomicBool`
//! 表达（取消当前项即终止整轮队列，剩余条目保留原状态）；ipatool 命令的
//! 输入/输出详细日志由 runner 侧（IpatoolClient sink）统一上报，队列只补
//! "请求许可"阶段里程碑，避免重复记录。

use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use crate::core::appcatalog::search_parser::SearchResult;
use crate::core::downloads::output_parser::DownloadOutputParser;
use crate::core::downloads::result_parser;
use crate::core::downloads::{DownloadQueueItem, DownloadQueueStatus};
use crate::core::ipatool::client::ClientError;
use crate::core::ipatool::response_parser::NormalizedText;
use crate::core::ipatool::result::IpatoolResult;
use crate::core::purchases::sync_service::{LogLevel, LogMessage};

/// 单个下载的执行入口（宿主实现：调用 `IpatoolClient::download_app` 并传入
/// 输出片段回调；测试用假实现）。
pub type DownloadRunner<'a> = &'a dyn Fn(
    &DownloadQueueItem,
    Option<&(dyn Fn(&str) + Sync)>,
    &AtomicBool,
    &mut dyn FnMut(LogMessage),
) -> Result<IpatoolResult, ClientError>;

/// 入队结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddQueueResult {
    Ignored,
    Added,
    Updated,
    Requeued,
}

/// 队列启动参数。
pub struct StartQueueParams<'a> {
    /// 输出目录（仅用于 StartQueue 日志参数）。
    pub output_directory: &'a str,
    /// mock 账户直通：条目直接标记成功，不调用 runner。
    pub is_mock: bool,
    pub cancel: &'a AtomicBool,
    pub runner: DownloadRunner<'a>,
    pub on_log: &'a mut dyn FnMut(LogMessage),
    pub on_change: &'a mut dyn FnMut(),
}

/// 下载队列：单飞行守卫 + 串行状态机。
#[derive(Debug, Default)]
pub struct DownloadQueueService {
    items: Mutex<Vec<DownloadQueueItem>>,
    is_running: AtomicBool,
}

impl DownloadQueueService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// 条目快照（宿主 UI 绑定用）。
    pub fn items_snapshot(&self) -> Vec<DownloadQueueItem> {
        self.items.lock().expect("queue lock").clone()
    }

    /// 从搜索结果入队；已存在则更新元数据，终态条目重新排队。
    pub fn add_or_update_from_search_result(
        &self,
        app: &SearchResult,
        on_log: &mut dyn FnMut(LogMessage),
        on_change: &mut dyn FnMut(),
    ) -> AddQueueResult {
        let bundle_id = app.bundle_id.trim().to_string();
        if bundle_id.is_empty() {
            return AddQueueResult::Ignored;
        }

        let mut items = self.items.lock().expect("queue lock");
        if let Some(existing) = items.iter_mut().find(|item| item.bundle_id == bundle_id) {
            existing.app_id = app.id.clone().unwrap_or_else(|| existing.app_id.clone());
            existing.name = app.name.clone().unwrap_or_else(|| existing.name.clone());
            existing.developer = app
                .developer
                .clone()
                .unwrap_or_else(|| existing.developer.clone());
            existing.version = app
                .version
                .clone()
                .unwrap_or_else(|| existing.version.clone());
            existing.price = app.price.clone();
            existing.artwork_url = app
                .artwork_url
                .clone()
                .unwrap_or_else(|| existing.artwork_url.clone());

            let requeued = matches!(
                existing.status,
                DownloadQueueStatus::Failed
                    | DownloadQueueStatus::Canceled
                    | DownloadQueueStatus::Success
            );
            if requeued {
                existing.status = DownloadQueueStatus::Pending;
                existing.last_message = "DownloadQueue/Status/Requeued".to_string();
            }

            let name = existing.name.clone();
            drop(items);
            on_log(LogMessage::key(
                LogLevel::Success,
                if requeued {
                    "DownloadQueue/Log/Requeued"
                } else {
                    "DownloadQueue/Log/Updated"
                },
                vec![name, bundle_id],
            ));
            on_change();
            return if requeued {
                AddQueueResult::Requeued
            } else {
                AddQueueResult::Updated
            };
        }

        let mut item = DownloadQueueItem::new(&bundle_id);
        item.app_id = app.id.clone().unwrap_or_default();
        item.name = app.name.clone().unwrap_or_else(|| bundle_id.clone());
        item.developer = app.developer.clone().unwrap_or_default();
        item.version = app.version.clone().unwrap_or_default();
        item.price = app.price.clone();
        item.artwork_url = app.artwork_url.clone().unwrap_or_default();
        let name = item.name.clone();
        items.push(item);
        drop(items);

        on_log(LogMessage::key(
            LogLevel::Success,
            "DownloadQueue/Log/Added",
            vec![name, bundle_id],
        ));
        on_change();
        AddQueueResult::Added
    }

    /// 按条件移除条目；运行中的下载中条目不可移除。返回移除数量。
    pub fn remove_items(
        &self,
        predicate: impl Fn(&DownloadQueueItem) -> bool,
        on_log: &mut dyn FnMut(LogMessage),
        on_change: &mut dyn FnMut(),
    ) -> i32 {
        let is_running = self.is_running();
        let mut items = self.items.lock().expect("queue lock");
        let before = items.len();
        items.retain(|item| {
            !(predicate(item) && !(is_running && item.status == DownloadQueueStatus::Downloading))
        });
        let removed = (before - items.len()) as i32;
        drop(items);

        if removed > 0 {
            on_log(LogMessage::key(
                LogLevel::Success,
                "DownloadQueue/Log/Removed",
                vec![removed.to_string()],
            ));
            on_change();
        }
        removed
    }

    /// 启动队列：串行处理 Pending/Failed/Canceled 条目，返回完成的数量。
    pub fn start_queue(&self, params: StartQueueParams<'_>) -> i32 {
        let StartQueueParams {
            output_directory,
            is_mock,
            cancel,
            runner,
            on_log,
            on_change,
        } = params;

        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            on_log(LogMessage::key(
                LogLevel::Info,
                "DownloadQueue/Log/AlreadyRunning",
                Vec::new(),
            ));
            return 0;
        }

        let initial_count = {
            let items = self.items.lock().expect("queue lock");
            items.iter().filter(|item| is_runnable_item(item)).count()
        };
        if initial_count == 0 {
            on_log(LogMessage::key(
                LogLevel::Tip,
                "DownloadQueue/Log/NoPendingItems",
                Vec::new(),
            ));
            self.is_running.store(false, Ordering::Relaxed);
            return 0;
        }

        self.is_running.store(true, Ordering::Relaxed);
        on_change();
        on_log(LogMessage::key(
            LogLevel::Info,
            "DownloadQueue/Log/StartQueue",
            vec![initial_count.to_string(), output_directory.to_string()],
        ));

        let mut completed = 0_i32;
        let mut processed = 0_i32;
        let mut processed_bundle_ids: HashSet<String> = HashSet::new();
        let mut queue_canceled = false;

        loop {
            if cancel.load(Ordering::Relaxed) {
                on_log(LogMessage::key(
                    LogLevel::Tip,
                    "DownloadQueue/Log/QueueCanceled",
                    Vec::new(),
                ));
                queue_canceled = true;
                break;
            }

            let index = {
                let items = self.items.lock().expect("queue lock");
                items.iter().position(|item| {
                    !processed_bundle_ids.contains(&item.bundle_id) && is_runnable_item(item)
                })
            };
            let Some(index) = index else { break };

            let item = {
                let mut items = self.items.lock().expect("queue lock");
                items[index].status = DownloadQueueStatus::Downloading;
                items[index].clone()
            };
            processed_bundle_ids.insert(item.bundle_id.clone());
            processed += 1;
            update_stage(
                &self.items,
                &item.bundle_id,
                "DownloadQueue/Status/Downloading",
                "DownloadQueue/Log/Downloading",
                &item.name,
                on_log,
                on_change,
            );

            if is_mock {
                self.finish_item(
                    &item.bundle_id,
                    DownloadQueueStatus::Success,
                    "DownloadQueue/Status/Success",
                );
                completed += 1;
                on_log(LogMessage::key(
                    LogLevel::Success,
                    "DownloadQueue/Log/MockSuccess",
                    vec![item.name.clone()],
                ));
                on_change();
                continue;
            }

            let context = Arc::new(ChunkContext::new(
                &self.items,
                &item.bundle_id,
                &item.name,
                cancel,
            ));
            let context_for_chunk = Arc::clone(&context);
            let on_chunk = move |chunk: &str| context_for_chunk.process(chunk);

            let run_result = runner(&item, Some(&on_chunk), cancel, on_log);

            // 冲刷输出解析器；请求许可阶段已在回调中实时更新条目。
            // ipatool 输入/输出日志由 runner 侧（IpatoolClient sink）统一上报，
            // 队列只补阶段里程碑，避免同一输出重复记录。
            let flush = context.flush();
            if flush.requesting_license {
                update_stage(
                    &self.items,
                    &item.bundle_id,
                    "DownloadQueue/Status/RequestingLicense",
                    "DownloadQueue/Log/RequestingLicense",
                    &item.name,
                    on_log,
                    on_change,
                );
            }
            for log in context.drain_logs() {
                on_log(log);
            }

            match run_result {
                Err(ClientError::Canceled) => {
                    self.finish_item(
                        &item.bundle_id,
                        DownloadQueueStatus::Canceled,
                        "DownloadQueue/Status/Canceled",
                    );
                    on_log(LogMessage::key(
                        LogLevel::Tip,
                        "DownloadQueue/Log/Canceled",
                        vec![item.name.clone()],
                    ));
                    on_log(LogMessage::key(
                        LogLevel::Tip,
                        "DownloadQueue/Log/QueueCanceled",
                        Vec::new(),
                    ));
                    queue_canceled = true;
                    break;
                }
                Ok(result) => {
                    if result_parser::is_success(&result) {
                        self.finish_item(
                            &item.bundle_id,
                            DownloadQueueStatus::Success,
                            "DownloadQueue/Status/Success",
                        );
                        completed += 1;
                        on_log(LogMessage::key(
                            LogLevel::Success,
                            "DownloadQueue/Log/Success",
                            vec![item.name.clone()],
                        ));
                    } else {
                        // 键名错误（超时/退出码）走专用日志键，避免键名嵌套进参数。
                        match result_parser::get_error_message(&result) {
                            NormalizedText::Raw(text) => {
                                self.finish_item(
                                    &item.bundle_id,
                                    DownloadQueueStatus::Failed,
                                    &text,
                                );
                                on_log(LogMessage::key(
                                    LogLevel::Error,
                                    "DownloadQueue/Log/Failed",
                                    vec![item.name.clone(), text],
                                ));
                            }
                            NormalizedText::Keyed { key, args } => {
                                self.finish_item(&item.bundle_id, DownloadQueueStatus::Failed, key);
                                let log_key = match key {
                                    "DownloadQueue/Error/Timeout" => {
                                        "DownloadQueue/Log/FailedTimeout"
                                    }
                                    "DownloadQueue/Error/ExitCode" => {
                                        "DownloadQueue/Log/FailedExitCode"
                                    }
                                    _ => "DownloadQueue/Log/Failed",
                                };
                                let mut log_args = vec![item.name.clone()];
                                log_args.extend(args);
                                on_log(LogMessage::key(LogLevel::Error, log_key, log_args));
                            }
                        }
                    }
                }
            }
            on_change();
        }

        if !queue_canceled {
            on_log(LogMessage::key(
                LogLevel::Info,
                "DownloadQueue/Log/Completed",
                vec![completed.to_string(), processed.to_string()],
            ));
        }

        self.is_running.store(false, Ordering::Relaxed);
        on_change();
        completed
    }
}

/// 下载期间的输出解析上下文：回调线程只做加锁更新，日志延迟到完成后上报。
/// 仅跟踪"请求许可"阶段里程碑；输出行的详细日志由 runner 侧统一上报。
struct ChunkContext<'a> {
    items: &'a Mutex<Vec<DownloadQueueItem>>,
    bundle_id: String,
    name: String,
    cancel: &'a AtomicBool,
    parser: Mutex<DownloadOutputParser>,
    pending_logs: Mutex<Vec<LogMessage>>,
}

impl<'a> ChunkContext<'a> {
    fn new(
        items: &'a Mutex<Vec<DownloadQueueItem>>,
        bundle_id: &str,
        name: &str,
        cancel: &'a AtomicBool,
    ) -> Self {
        Self {
            items,
            bundle_id: bundle_id.to_string(),
            name: name.to_string(),
            cancel,
            parser: Mutex::new(DownloadOutputParser::new()),
            pending_logs: Mutex::new(Vec::new()),
        }
    }

    fn process(&self, chunk: &str) {
        if self.cancel.load(Ordering::Relaxed) {
            return;
        }

        let update = self
            .parser
            .lock()
            .expect("parser lock")
            .process_chunk(Some(chunk));
        if update.requesting_license {
            self.set_stage("DownloadQueue/Status/RequestingLicense");
            self.pending_logs
                .lock()
                .expect("pending lock")
                .push(LogMessage::key(
                    LogLevel::Info,
                    "DownloadQueue/Log/RequestingLicense",
                    vec![self.name.clone()],
                ));
        }
    }

    fn set_stage(&self, stage_key: &str) {
        let mut items = self.items.lock().expect("queue lock");
        if let Some(item) = items
            .iter_mut()
            .find(|item| item.bundle_id == self.bundle_id)
        {
            if item.last_message != stage_key {
                item.last_message = stage_key.to_string();
            }
        }
    }

    fn flush(&self) -> crate::core::downloads::output_parser::DownloadOutputUpdate {
        let update = self.parser.lock().expect("parser lock").flush();
        if update.requesting_license {
            self.set_stage("DownloadQueue/Status/RequestingLicense");
        }
        update
    }

    fn drain_logs(&self) -> Vec<LogMessage> {
        std::mem::take(&mut *self.pending_logs.lock().expect("pending lock"))
    }
}

impl DownloadQueueService {
    fn finish_item(&self, bundle_id: &str, status: DownloadQueueStatus, message: &str) {
        let mut items = self.items.lock().expect("queue lock");
        if let Some(item) = items.iter_mut().find(|item| item.bundle_id == bundle_id) {
            item.status = status;
            item.last_message = message.to_string();
        }
    }
}

fn is_runnable_item(item: &DownloadQueueItem) -> bool {
    matches!(
        item.status,
        DownloadQueueStatus::Pending | DownloadQueueStatus::Failed | DownloadQueueStatus::Canceled
    )
}

fn update_stage(
    items: &Mutex<Vec<DownloadQueueItem>>,
    bundle_id: &str,
    stage_key: &str,
    log_key: &'static str,
    name: &str,
    on_log: &mut dyn FnMut(LogMessage),
    on_change: &mut dyn FnMut(),
) {
    {
        let mut items = items.lock().expect("queue lock");
        if let Some(item) = items.iter_mut().find(|item| item.bundle_id == bundle_id) {
            if item.last_message == stage_key {
                return;
            }
            item.last_message = stage_key.to_string();
        }
    }
    on_log(LogMessage::key(
        LogLevel::Info,
        log_key,
        vec![name.to_string()],
    ));
    on_change();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ipatool::response_parser::NormalizedText;
    use std::sync::Arc;

    fn search_result(bundle_id: &str) -> SearchResult {
        SearchResult {
            bundle_id: bundle_id.to_string(),
            id: Some("1".to_string()),
            name: Some(format!("App {bundle_id}")),
            developer: Some("Dev".to_string()),
            artwork_url: None,
            price: "free".to_string(),
            version: Some("1.0".to_string()),
            purchased: "purchased".to_string(),
        }
    }

    fn noop_log(_: LogMessage) {}
    fn noop_change() {}

    #[test]
    fn add_or_update_adds_updates_and_requeues() {
        let service = DownloadQueueService::new();

        assert_eq!(
            service.add_or_update_from_search_result(
                &search_result("com.a"),
                &mut noop_log,
                &mut noop_change
            ),
            AddQueueResult::Added
        );
        assert_eq!(
            service.add_or_update_from_search_result(
                &search_result("com.a"),
                &mut noop_log,
                &mut noop_change
            ),
            AddQueueResult::Updated
        );

        service.finish_item(
            "com.a",
            DownloadQueueStatus::Success,
            "DownloadQueue/Status/Success",
        );
        assert_eq!(
            service.add_or_update_from_search_result(
                &search_result("com.a"),
                &mut noop_log,
                &mut noop_change
            ),
            AddQueueResult::Requeued
        );

        let items = service.items_snapshot();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, DownloadQueueStatus::Pending);
    }

    #[test]
    fn add_or_update_ignores_blank_bundle_ids() {
        let service = DownloadQueueService::new();
        let app = search_result("  ");

        assert_eq!(
            service.add_or_update_from_search_result(&app, &mut noop_log, &mut noop_change),
            AddQueueResult::Ignored
        );
    }

    #[test]
    fn start_queue_processes_pending_items_and_reports_completion() {
        let service = DownloadQueueService::new();
        service.add_or_update_from_search_result(
            &search_result("com.a"),
            &mut noop_log,
            &mut noop_change,
        );
        service.add_or_update_from_search_result(
            &search_result("com.b"),
            &mut noop_log,
            &mut noop_change,
        );

        let cancel = AtomicBool::new(false);
        let runner = |item: &DownloadQueueItem,
                      _: Option<&(dyn Fn(&str) + Sync)>,
                      _: &AtomicBool,
                      _: &mut dyn FnMut(LogMessage)|
         -> Result<IpatoolResult, ClientError> {
            if item.bundle_id == "com.b" {
                return Ok(IpatoolResult::from_streams(
                    NormalizedText::Raw(String::new()),
                    NormalizedText::Raw("permission denied".to_string()),
                    1,
                ));
            }
            Ok(IpatoolResult::from_streams(
                NormalizedText::Raw("{\"success\":true}".to_string()),
                NormalizedText::Raw(String::new()),
                0,
            ))
        };

        let mut logs: Vec<LogMessage> = Vec::new();
        let mut changes = 0;
        {
            let mut log_sink = |log: LogMessage| logs.push(log);
            let mut change_sink = || changes += 1;
            let completed = service.start_queue(StartQueueParams {
                output_directory: "C:\\Downloads",
                is_mock: false,
                cancel: &cancel,
                runner: &runner,
                on_log: &mut log_sink,
                on_change: &mut change_sink,
            });
            assert_eq!(completed, 1);
        }

        let items = service.items_snapshot();
        assert_eq!(items[0].status, DownloadQueueStatus::Success);
        assert_eq!(items[1].status, DownloadQueueStatus::Failed);
        assert_eq!(items[1].last_message, "permission denied");
        assert!(changes > 0);
        assert!(logs.iter().any(|log| log.message
            == NormalizedText::Keyed {
                key: "DownloadQueue/Log/Failed",
                args: vec!["App com.b".to_string(), "permission denied".to_string()]
            }));
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "DownloadQueue/Log/Completed")
        );
    }

    #[test]
    fn start_queue_mock_flow_marks_all_success() {
        let service = DownloadQueueService::new();
        service.add_or_update_from_search_result(
            &search_result("com.a"),
            &mut noop_log,
            &mut noop_change,
        );

        let cancel = AtomicBool::new(false);
        let runner = |_item: &DownloadQueueItem,
                      _: Option<&(dyn Fn(&str) + Sync)>,
                      _: &AtomicBool,
                      _: &mut dyn FnMut(LogMessage)|
         -> Result<IpatoolResult, ClientError> {
            panic!("mock flow must not invoke runner");
        };

        let mut logs: Vec<LogMessage> = Vec::new();
        let mut change_sink = || {};
        let mut log_sink = |log: LogMessage| logs.push(log);
        let completed = service.start_queue(StartQueueParams {
            output_directory: "C:\\Downloads",
                is_mock: true,
                cancel: &cancel,
            runner: &runner,
            on_log: &mut log_sink,
            on_change: &mut change_sink,
        });

        assert_eq!(completed, 1);
        assert_eq!(
            service.items_snapshot()[0].status,
            DownloadQueueStatus::Success
        );
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "DownloadQueue/Log/MockSuccess")
        );
    }

    #[test]
    fn start_queue_delivers_chunk_callback_and_reports_license_stage() {
        let service = DownloadQueueService::new();
        service.add_or_update_from_search_result(
            &search_result("com.a"),
            &mut noop_log,
            &mut noop_change,
        );

        let cancel = AtomicBool::new(false);
        let runner = |_item: &DownloadQueueItem,
                      on_chunk: Option<&(dyn Fn(&str) + Sync)>,
                      _: &AtomicBool,
                      _: &mut dyn FnMut(LogMessage)|
         -> Result<IpatoolResult, ClientError> {
            if let Some(on_chunk) = on_chunk {
                on_chunk("{\"message\":\"purchase\"}\n");
                on_chunk("downloading 40%\n");
            }
            Ok(IpatoolResult::from_streams(
                NormalizedText::Raw("{\"success\":true}".to_string()),
                NormalizedText::Raw(String::new()),
                0,
            ))
        };

        let mut logs: Vec<LogMessage> = Vec::new();
        let mut change_sink = || {};
        let mut log_sink = |log: LogMessage| logs.push(log);
        let completed = service.start_queue(StartQueueParams {
            output_directory: "C:\\Downloads",
            is_mock: false,
            cancel: &cancel,
            runner: &runner,
            on_log: &mut log_sink,
            on_change: &mut change_sink,
        });

        assert_eq!(completed, 1);
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "DownloadQueue/Log/RequestingLicense")
        );
        // 队列自身不再上报 ipatool 输入/输出详细行（由 runner 侧 sink 统一处理）
        assert!(!logs.iter().any(|log| log.level == LogLevel::Ipatool));
    }

    #[test]
    fn start_queue_cancellation_marks_current_item_and_stops() {
        let service = DownloadQueueService::new();
        service.add_or_update_from_search_result(
            &search_result("com.a"),
            &mut noop_log,
            &mut noop_change,
        );
        service.add_or_update_from_search_result(
            &search_result("com.b"),
            &mut noop_log,
            &mut noop_change,
        );

        let cancel = AtomicBool::new(false);
        let runner_cancel = Arc::new(AtomicBool::new(false));
        let runner_cancel_for_runner = Arc::clone(&runner_cancel);
        let runner = move |_item: &DownloadQueueItem,
                           _: Option<&(dyn Fn(&str) + Sync)>,
                           _: &AtomicBool,
                           _: &mut dyn FnMut(LogMessage)|
              -> Result<IpatoolResult, ClientError> {
            runner_cancel_for_runner.store(true, Ordering::Relaxed);
            Err(ClientError::Canceled)
        };

        let mut change_sink = || {};
        let mut log_sink = |_: LogMessage| {};
        let completed = service.start_queue(StartQueueParams {
            output_directory: "C:\\Downloads",
            is_mock: false,
            cancel: &cancel,
            runner: &runner,
            on_log: &mut log_sink,
            on_change: &mut change_sink,
        });

        assert_eq!(completed, 0);
        let items = service.items_snapshot();
        assert_eq!(items[0].status, DownloadQueueStatus::Canceled);
        assert_eq!(items[1].status, DownloadQueueStatus::Pending);
    }

    #[test]
    fn start_queue_without_runnable_items_reports_no_pending() {
        let service = DownloadQueueService::new();
        let cancel = AtomicBool::new(false);
        let runner = |_item: &DownloadQueueItem,
                      _: Option<&(dyn Fn(&str) + Sync)>,
                      _: &AtomicBool,
                      _: &mut dyn FnMut(LogMessage)|
         -> Result<IpatoolResult, ClientError> { unreachable!() };

        let mut logs: Vec<LogMessage> = Vec::new();
        let mut change_sink = || {};
        let mut log_sink = |log: LogMessage| logs.push(log);
        let completed = service.start_queue(StartQueueParams {
            output_directory: "C:\\Downloads",
            is_mock: false,
            cancel: &cancel,
            runner: &runner,
            on_log: &mut log_sink,
            on_change: &mut change_sink,
        });

        assert_eq!(completed, 0);
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "DownloadQueue/Log/NoPendingItems")
        );
    }

    #[test]
    fn start_queue_failed_exit_code_uses_dedicated_log_key() {
        let service = DownloadQueueService::new();
        service.add_or_update_from_search_result(
            &search_result("com.a"),
            &mut noop_log,
            &mut noop_change,
        );

        let cancel = AtomicBool::new(false);
        let runner = |_item: &DownloadQueueItem,
                      _: Option<&(dyn Fn(&str) + Sync)>,
                      _: &AtomicBool,
                      _: &mut dyn FnMut(LogMessage)|
         -> Result<IpatoolResult, ClientError> {
            Ok(IpatoolResult::from_streams(
                NormalizedText::Raw(String::new()),
                NormalizedText::Raw(String::new()),
                3,
            ))
        };

        let mut logs: Vec<LogMessage> = Vec::new();
        let mut change_sink = || {};
        let mut log_sink = |log: LogMessage| logs.push(log);
        service.start_queue(StartQueueParams {
            output_directory: "C:\\Downloads",
            is_mock: false,
            cancel: &cancel,
            runner: &runner,
            on_log: &mut log_sink,
            on_change: &mut change_sink,
        });

        assert!(logs.iter().any(|log| log.message
            == NormalizedText::Keyed {
                key: "DownloadQueue/Log/FailedExitCode",
                args: vec!["App com.a".to_string(), "3".to_string()],
            }));
        assert_eq!(
            service.items_snapshot()[0].last_message,
            "DownloadQueue/Error/ExitCode"
        );
    }

    #[test]
    fn remove_items_skips_downloading_while_running() {
        let service = DownloadQueueService::new();
        service.add_or_update_from_search_result(
            &search_result("com.a"),
            &mut noop_log,
            &mut noop_change,
        );
        service.finish_item(
            "com.a",
            DownloadQueueStatus::Downloading,
            "DownloadQueue/Status/Downloading",
        );

        service.is_running.store(true, Ordering::Relaxed);
        assert_eq!(
            service.remove_items(|_| true, &mut noop_log, &mut noop_change),
            0
        );
        assert_eq!(service.items_snapshot().len(), 1);

        service.is_running.store(false, Ordering::Relaxed);
        assert_eq!(
            service.remove_items(|_| true, &mut noop_log, &mut noop_change),
            1
        );
        assert!(service.items_snapshot().is_empty());
    }
}
