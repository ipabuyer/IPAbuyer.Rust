//! 已购买列表同步服务。
//!
//! 移植自主仓库 `PurchaseSyncService`：分页拉取 `list-purchases`（每页 100，
//! 为 ipatool 单页上限）、逐页写入并统一标记为已购买；仅由宿主手动触发，
//! 单飞行守卫避免并发同步。持久化经 [`PurchaseStore`] trait 注入（阶段 3 由
//! rusqlite 实现），页拉取经 [`ListPurchasesFetcher`] 注入（由 [`IpatoolClient`]
//! 实现，测试可用假实现替换）。

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::core::ipatool::client::{ClientError, IpatoolClient};
use crate::core::ipatool::response_parser::NormalizedText;
use crate::core::ipatool::result::IpatoolResult;
use crate::core::purchases::owned_apps_page_parser;

/// list-purchases 单页数量上限（受 ipatool 限制不得超过 100）。
pub const PAGE_SIZE: i64 = 100;

/// 日志等级（对齐主应用 `UiLogLevel`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Tip,
    Success,
    Error,
    /// ipatool 原文输出行。
    Ipatool,
}

/// 待宿主本地化的日志条目：消息为键名（带参数）或原文（如 ipatool 输出行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogMessage {
    pub level: LogLevel,
    pub message: NormalizedText,
}

impl LogMessage {
    /// 键名日志（宿主用 resw 渲染）。
    pub fn key(level: LogLevel, key: &'static str, args: Vec<String>) -> Self {
        Self {
            level,
            message: NormalizedText::Keyed { key, args },
        }
    }

    /// 原文日志（宿主原样展示）。
    pub fn raw(level: LogLevel, message: String) -> Self {
        Self {
            level,
            message: NormalizedText::Raw(message),
        }
    }
}

/// 同步持久化接口（阶段 3 由 rusqlite 落地）。
pub trait PurchaseStore {
    /// 批量将 App 标记为已购买（单事务）；记录归属指定平台。
    fn bulk_mark_purchased(&mut self, bundle_ids: &[String], account: &str, platform: &str);
    /// 记录一次同步尝试；失败时保留原成功时间。
    fn record_sync_attempt(&mut self, account: &str, succeeded: bool);
}

/// list-purchases 页拉取接口（由 `IpatoolClient` 实现）。
/// `platform` 为 `None` 时使用 ipatool 缺省平台（不传参，兼容 2.5.0）。
pub trait ListPurchasesFetcher {
    #[allow(clippy::too_many_arguments)]
    fn fetch(
        &self,
        max_results: i64,
        page: i64,
        passphrase: Option<&str>,
        cancel: &AtomicBool,
        platform: Option<&str>,
        on_log: Option<&mut dyn FnMut(LogMessage)>,
    ) -> Result<IpatoolResult, ClientError>;
}

impl ListPurchasesFetcher for IpatoolClient {
    fn fetch(
        &self,
        max_results: i64,
        page: i64,
        passphrase: Option<&str>,
        cancel: &AtomicBool,
        platform: Option<&str>,
        on_log: Option<&mut dyn FnMut(LogMessage)>,
    ) -> Result<IpatoolResult, ClientError> {
        IpatoolClient::list_purchases(self, max_results, page, passphrase, cancel, on_log, platform)
    }
}

/// 同步结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    Completed { synced: i64, total: i64 },
    AlreadyRunning,
    Canceled,
    InvalidAccount,
    Failed { message: String },
}

/// 同步服务：单飞行守卫 + 分页拉取 + 逐页写入。
#[derive(Debug, Default)]
pub struct PurchaseSyncService {
    is_running: AtomicBool,
}

impl PurchaseSyncService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// 执行全量同步；`on_progress(synced, total)` 与 `on_log` 在调用线程上回调。
    /// `detailed_log` 开启时，list-purchases 命令行与输出行也经 `on_log` 上报。
    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &self,
        account: &str,
        passphrase: Option<&str>,
        fetcher: &dyn ListPurchasesFetcher,
        store: &mut dyn PurchaseStore,
        cancel: &AtomicBool,
        detailed_log: bool,
        on_progress: &mut dyn FnMut(i64, i64),
        on_log: &mut dyn FnMut(LogMessage),
    ) -> SyncOutcome {
        if account.trim().is_empty() {
            return SyncOutcome::InvalidAccount;
        }

        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            on_log(LogMessage::key(
                LogLevel::Tip,
                "PurchaseSync/Log/AlreadyRunning",
                Vec::new(),
            ));
            return SyncOutcome::AlreadyRunning;
        }

        let outcome = self.run(
            account,
            passphrase,
            fetcher,
            store,
            cancel,
            detailed_log,
            on_progress,
            on_log,
        );
        self.is_running.store(false, Ordering::Relaxed);
        outcome
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        account: &str,
        passphrase: Option<&str>,
        fetcher: &dyn ListPurchasesFetcher,
        store: &mut dyn PurchaseStore,
        cancel: &AtomicBool,
        detailed_log: bool,
        on_progress: &mut dyn FnMut(i64, i64),
        on_log: &mut dyn FnMut(LogMessage),
    ) -> SyncOutcome {
        // 内部标记：全部轮次走完（数值以累计偏移为准，见函数末尾）。
        const COMPLETED: SyncOutcome = SyncOutcome::Completed { synced: 0, total: 0 };

        on_log(LogMessage::key(
            LogLevel::Info,
            "PurchaseSync/Log/Start",
            vec![account.to_string()],
        ));

        let mut synced_total = 0_i64;
        let mut total_total = 0_i64;

        // 第一轮：iOS（ipatool 缺省平台，不传 --platform，兼容自定义 2.5.0）。
        let outcome = match self.run_platform(
            account,
            passphrase,
            fetcher,
            store,
            cancel,
            detailed_log,
            None,
            &mut synced_total,
            &mut total_total,
            on_progress,
            on_log,
        ) {
            SyncOutcome::Completed { .. } => {
                // 第二轮：Mac App Store。失败仅记录并跳过——自定义 ipatool
                // 2.5.0 没有 --platform 参数，不应拖垮整次同步。
                match self.run_platform(
                    account,
                    passphrase,
                    fetcher,
                    store,
                    cancel,
                    detailed_log,
                    Some(crate::core::platform::MACOS),
                    &mut synced_total,
                    &mut total_total,
                    on_progress,
                    on_log,
                ) {
                    SyncOutcome::Completed { .. } => COMPLETED,
                    SyncOutcome::Canceled => SyncOutcome::Canceled,
                    SyncOutcome::Failed { message } => {
                        on_log(LogMessage::key(
                            LogLevel::Tip,
                            "PurchaseSync/Log/MacosRoundFailed",
                            vec![message],
                        ));
                        COMPLETED
                    }
                    other => other,
                }
            }
            other => other,
        };

        let succeeded = matches!(outcome, SyncOutcome::Completed { .. });
        store.record_sync_attempt(account, succeeded);
        match outcome {
            SyncOutcome::Completed { .. } => {
                on_log(LogMessage::key(
                    LogLevel::Success,
                    "PurchaseSync/Log/Completed",
                    vec![synced_total.to_string(), total_total.to_string()],
                ));
                SyncOutcome::Completed {
                    synced: synced_total,
                    total: total_total,
                }
            }
            other => other,
        }
    }

    /// 单平台分页拉取；进度在全局偏移（已完成平台）之上累加。
    /// 尝试记录（成功/失败）由 [`Self::run`] 统一写一次。
    #[allow(clippy::too_many_arguments)]
    fn run_platform(
        &self,
        account: &str,
        passphrase: Option<&str>,
        fetcher: &dyn ListPurchasesFetcher,
        store: &mut dyn PurchaseStore,
        cancel: &AtomicBool,
        detailed_log: bool,
        platform: Option<&'static str>,
        synced_offset: &mut i64,
        total_offset: &mut i64,
        on_progress: &mut dyn FnMut(i64, i64),
        on_log: &mut dyn FnMut(LogMessage),
    ) -> SyncOutcome {
        let store_platform = platform.unwrap_or(crate::core::platform::IOS);
        let mut page = 1_i64;
        let mut synced = 0_i64;
        let mut total = -1_i64;

        loop {
            if cancel.load(Ordering::Relaxed) {
                on_log(LogMessage::key(
                    LogLevel::Tip,
                    "PurchaseSync/Log/Canceled",
                    Vec::new(),
                ));
                return SyncOutcome::Canceled;
            }

            let fetch_result = {
                let mut command_sink = |log: LogMessage| on_log(log);
                let sink: Option<&mut dyn FnMut(LogMessage)> = if detailed_log {
                    Some(&mut command_sink)
                } else {
                    None
                };
                fetcher.fetch(PAGE_SIZE, page, passphrase, cancel, platform, sink)
            };
            let result = match fetch_result {
                Ok(result) => result,
                Err(ClientError::Canceled) => {
                    on_log(LogMessage::key(
                        LogLevel::Tip,
                        "PurchaseSync/Log/Canceled",
                        Vec::new(),
                    ));
                    return SyncOutcome::Canceled;
                }
            };

            // 超时没有可解析输出：失败消息退化为安全命令标签。
            if result.timed_out {
                let message = FALLBACK_COMMAND_LABEL.to_string();
                on_log(LogMessage::key(
                    LogLevel::Error,
                    "PurchaseSync/Log/Failed",
                    vec![message.clone()],
                ));
                return SyncOutcome::Failed { message };
            }

            let parsed = owned_apps_page_parser::parse(Some(&result.output_or_error_raw()));
            if !parsed.success {
                let message = parsed
                    .error_message
                    .filter(|message| !message.trim().is_empty())
                    .unwrap_or_else(|| FALLBACK_COMMAND_LABEL.to_string());
                on_log(LogMessage::key(
                    LogLevel::Error,
                    "PurchaseSync/Log/Failed",
                    vec![message.clone()],
                ));
                return SyncOutcome::Failed { message };
            }

            if total < 0 {
                total = parsed.total_count;
            }

            if !parsed.bundle_ids.is_empty() {
                store.bulk_mark_purchased(&parsed.bundle_ids, account, store_platform);
                synced += parsed.bundle_ids.len() as i64;
                on_progress(
                    *synced_offset + synced,
                    *total_offset + total.max(synced),
                );
                on_log(LogMessage::key(
                    LogLevel::Info,
                    "PurchaseSync/Log/Progress",
                    vec![
                        (*synced_offset + synced).to_string(),
                        (*total_offset + total.max(synced)).to_string(),
                    ],
                ));
            }

            if synced >= total || parsed.bundle_ids.is_empty() {
                break;
            }

            page += 1;
        }

        *synced_offset += synced;
        *total_offset += total.max(0);
        SyncOutcome::Completed {
            synced,
            total: total.max(0),
        }
    }
}

/// 失败消息的兜底值：超时/空输出时无原文可展示，退化为安全命令标签。
const FALLBACK_COMMAND_LABEL: &str = "ipatool list-purchases";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ipatool::response_parser::NormalizedText;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

    #[derive(Default)]
    struct FakeStore {
        /// (平台, bundle_ids)
        bulk_calls: Vec<(String, Vec<String>)>,
        attempts: Vec<(String, bool)>,
    }

    impl PurchaseStore for FakeStore {
        fn bulk_mark_purchased(&mut self, bundle_ids: &[String], _account: &str, platform: &str) {
            self.bulk_calls
                .push((platform.to_string(), bundle_ids.to_vec()));
        }

        fn record_sync_attempt(&mut self, account: &str, succeeded: bool) {
            self.attempts.push((account.to_string(), succeeded));
        }
    }

    struct FakeFetcher {
        pages: Vec<IpatoolResult>,
        calls: AtomicUsize,
    }

    impl FakeFetcher {
        fn new(pages: Vec<IpatoolResult>) -> Self {
            Self {
                pages,
                calls: AtomicUsize::new(0),
            }
        }

        fn page(output: &str) -> IpatoolResult {
            IpatoolResult::from_streams(
                NormalizedText::Raw(output.to_string()),
                NormalizedText::Raw(String::new()),
                0,
            )
        }

        fn empty_page() -> IpatoolResult {
            Self::page(r#"{"level":"info","count":0,"totalCount":0,"apps":[]}"#)
        }
    }

    impl ListPurchasesFetcher for FakeFetcher {
        fn fetch(
            &self,
            _max_results: i64,
            page: i64,
            _passphrase: Option<&str>,
            _cancel: &AtomicBool,
            _platform: Option<&str>,
            _on_log: Option<&mut dyn FnMut(LogMessage)>,
        ) -> Result<IpatoolResult, ClientError> {
            let index = self.calls.fetch_add(1, Ordering::Relaxed);
            let _ = page;
            // 页序耗尽后返回空页（第二轮平台无更多内容即结束）。
            Ok(self
                .pages
                .get(index)
                .cloned()
                .unwrap_or_else(|| Self::empty_page()))
        }
    }

    /// 平台感知的假拉取器：macOS 轮固定失败（模拟自定义 ipatool 2.5.0）。
    struct MacosFailingFetcher {
        ios_pages: Vec<IpatoolResult>,
        calls: AtomicUsize,
    }

    impl ListPurchasesFetcher for MacosFailingFetcher {
        fn fetch(
            &self,
            _max_results: i64,
            _page: i64,
            _passphrase: Option<&str>,
            _cancel: &AtomicBool,
            platform: Option<&str>,
            _on_log: Option<&mut dyn FnMut(LogMessage)>,
        ) -> Result<IpatoolResult, ClientError> {
            if platform == Some(crate::core::platform::MACOS) {
                return Ok(FakeFetcher::page(
                    r#"{"level":"error","error":"unknown flag: --platform","success":false}"#,
                ));
            }
            let index = self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(self
                .ios_pages
                .get(index)
                .cloned()
                .unwrap_or_else(|| FakeFetcher::empty_page()))
        }
    }

    fn success_page(count: i64, total: i64, prefix: &str) -> IpatoolResult {
        let apps: Vec<String> = (0..count)
            .map(|index| format!(r#"{{"bundleID":"{prefix}.{index}"}}"#))
            .collect();
        FakeFetcher::page(&format!(
            r#"{{"level":"info","count":{count},"totalCount":{total},"page":1,"apps":[{}]}}"#,
            apps.join(",")
        ))
    }

    #[test]
    fn sync_pages_until_total_reached_and_records_success() {
        let service = PurchaseSyncService::new();
        let mut store = FakeStore::default();
        let fetcher = FakeFetcher::new(vec![
            success_page(2, 3, "com.a"),
            success_page(1, 3, "com.b"),
        ]);
        let cancel = AtomicBool::new(false);
        let mut progress_events: Vec<(i64, i64)> = Vec::new();
        let mut logs: Vec<LogMessage> = Vec::new();

        let outcome = service.sync(
            "user@example.com",
            None,
            &fetcher,
            &mut store,
            &cancel,
            false,
            &mut |synced, total| progress_events.push((synced, total)),
            &mut |message| logs.push(message),
        );

        assert_eq!(
            outcome,
            SyncOutcome::Completed {
                synced: 3,
                total: 3
            }
        );
        assert_eq!(store.bulk_calls.len(), 2);
        assert_eq!(store.attempts, vec![("user@example.com".to_string(), true)]);
        assert_eq!(progress_events, vec![(2, 3), (3, 3)]);
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "PurchaseSync/Log/Start")
        );
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "PurchaseSync/Log/Progress")
        );
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "PurchaseSync/Log/Completed")
        );
    }

    #[test]
    fn sync_empty_account_is_invalid() {
        let service = PurchaseSyncService::new();
        let mut store = FakeStore::default();
        let fetcher = FakeFetcher::new(Vec::new());
        let cancel = AtomicBool::new(false);
        let mut ignored = |_: i64, _: i64| {};
        let mut log_sink = |_: LogMessage| {};

        assert_eq!(
            service.sync(
                "  ",
                None,
                &fetcher,
                &mut store,
                &cancel,
                false,
                &mut ignored,
                &mut log_sink
            ),
            SyncOutcome::InvalidAccount
        );
    }

    #[test]
    fn sync_error_page_records_failure_with_message() {
        let service = PurchaseSyncService::new();
        let mut store = FakeStore::default();
        let fetcher = FakeFetcher::new(vec![FakeFetcher::page(
            r#"{"level":"error","error":"max results must not exceed 100","success":false}"#,
        )]);
        let cancel = AtomicBool::new(false);
        let mut ignored = |_: i64, _: i64| {};
        let mut log_sink = |_: LogMessage| {};

        let outcome = service.sync(
            "user",
            None,
            &fetcher,
            &mut store,
            &cancel,
            false,
            &mut ignored,
            &mut log_sink,
        );

        assert_eq!(
            outcome,
            SyncOutcome::Failed {
                message: "max results must not exceed 100".to_string()
            }
        );
        assert_eq!(store.attempts, vec![("user".to_string(), false)]);
    }

    #[test]
    fn sync_cancellation_records_failed_attempt() {
        let service = PurchaseSyncService::new();
        let mut store = FakeStore::default();
        let fetcher = FakeFetcher::new(Vec::new());
        let cancel = AtomicBool::new(true);
        let mut ignored = |_: i64, _: i64| {};
        let mut log_sink = |_: LogMessage| {};

        assert_eq!(
            service.sync(
                "user",
                None,
                &fetcher,
                &mut store,
                &cancel,
                false,
                &mut ignored,
                &mut log_sink
            ),
            SyncOutcome::Canceled
        );
        assert_eq!(store.attempts, vec![("user".to_string(), false)]);
    }

    #[test]
    fn sync_covers_both_platforms_and_tags_records() {
        let service = PurchaseSyncService::new();
        let mut store = FakeStore::default();
        let fetcher = FakeFetcher::new(vec![
            success_page(1, 1, "com.ios"),
            success_page(2, 2, "com.mac"),
        ]);
        let cancel = AtomicBool::new(false);
        let mut progress_events: Vec<(i64, i64)> = Vec::new();
        let mut logs: Vec<LogMessage> = Vec::new();

        let outcome = service.sync(
            "user",
            None,
            &fetcher,
            &mut store,
            &cancel,
            false,
            &mut |synced, total| progress_events.push((synced, total)),
            &mut |message| logs.push(message),
        );

        assert_eq!(
            outcome,
            SyncOutcome::Completed {
                synced: 3,
                total: 3
            }
        );
        // 第一轮 iOS 一页、第二轮 macOS 一页，平台标记分别正确。
        assert_eq!(
            store.bulk_calls,
            vec![
                ("ios".to_string(), vec!["com.ios.0".to_string()]),
                (
                    "macos".to_string(),
                    vec!["com.mac.0".to_string(), "com.mac.1".to_string()]
                ),
            ]
        );
        assert_eq!(store.attempts, vec![("user".to_string(), true)]);
        // macOS 轮进度在 iOS 偏移（1, 1）之上累加。
        assert_eq!(progress_events, vec![(1, 1), (3, 3)]);
    }

    #[test]
    fn macos_round_failure_is_tolerated_and_logged() {
        let service = PurchaseSyncService::new();
        let mut store = FakeStore::default();
        let fetcher = MacosFailingFetcher {
            ios_pages: vec![success_page(1, 1, "com.a")],
            calls: AtomicUsize::new(0),
        };
        let cancel = AtomicBool::new(false);
        let mut logs: Vec<LogMessage> = Vec::new();
        let mut ignored = |_: i64, _: i64| {};

        let outcome = service.sync(
            "user",
            None,
            &fetcher,
            &mut store,
            &cancel,
            false,
            &mut ignored,
            &mut |message| logs.push(message),
        );

        // macOS 轮失败不拖垮整次同步（兼容自定义 ipatool 2.5.0）。
        assert_eq!(
            outcome,
            SyncOutcome::Completed {
                synced: 1,
                total: 1
            }
        );
        assert_eq!(store.attempts, vec![("user".to_string(), true)]);
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "PurchaseSync/Log/MacosRoundFailed")
        );
    }

    #[test]
    fn sync_is_single_flight() {
        let service = Arc::new(PurchaseSyncService::new());
        let cancel = Arc::new(AtomicBool::new(false));
        let (started_sender, started_receiver) = mpsc::channel::<()>();
        let release = Arc::new(AtomicBool::new(false));

        struct BlockingFetcher {
            started: mpsc::Sender<()>,
            release: Arc<AtomicBool>,
        }

        impl ListPurchasesFetcher for BlockingFetcher {
            fn fetch(
                &self,
                _max_results: i64,
                _page: i64,
                _passphrase: Option<&str>,
                _cancel: &AtomicBool,
                _platform: Option<&str>,
                _on_log: Option<&mut dyn FnMut(LogMessage)>,
            ) -> Result<IpatoolResult, ClientError> {
                self.started.send(()).unwrap();
                while !self.release.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Ok(FakeFetcher::page(
                    r#"{"level":"info","count":0,"totalCount":0,"apps":[]}"#,
                ))
            }
        }

        let first_service = Arc::clone(&service);
        let first_cancel = Arc::clone(&cancel);
        let first_release = Arc::clone(&release);
        let first = std::thread::spawn(move || {
            let mut store = FakeStore::default();
            let mut ignored = |_: i64, _: i64| {};
            let mut log_sink = |_: LogMessage| {};
            let fetcher = BlockingFetcher {
                started: started_sender,
                release: first_release,
            };
            first_service.sync(
                "user",
                None,
                &fetcher,
                &mut store,
                &first_cancel,
                false,
                &mut ignored,
                &mut log_sink,
            )
        });

        started_receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(service.is_running());

        let mut store = FakeStore::default();
        let fetcher = FakeFetcher::new(Vec::new());
        let mut ignored = |_: i64, _: i64| {};
        let mut logs: Vec<LogMessage> = Vec::new();
        let mut log_sink = |message: LogMessage| logs.push(message);
        assert_eq!(
            service.sync(
                "user",
                None,
                &fetcher,
                &mut store,
                &cancel,
                false,
                &mut ignored,
                &mut log_sink
            ),
            SyncOutcome::AlreadyRunning
        );
        assert!(
            logs.iter()
                .any(|log| log.message.key() == "PurchaseSync/Log/AlreadyRunning")
        );

        release.store(true, Ordering::Relaxed);
        assert!(matches!(
            first.join().unwrap(),
            SyncOutcome::Completed { .. }
        ));
        assert!(!service.is_running());
    }
}
