//! 购买命令：前置策略（宿主侧）+ core 购买执行与响应解释。
//! 执行 ipatool 子进程（最长 2 分钟），命令标记 `(async)` 在独立线程运行，
//! 避免同步命令阻塞主线程冻结 UI/光标。

use serde::Serialize;
use std::sync::atomic::AtomicBool;
use tauri::State;

use crate::core::ipatool::client::IpatoolClient;
use crate::core::purchases::response_interpreter::{self, PurchaseOutcome};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseDto {
    pub bundle_id: String,
    /// Purchased / AlreadyOwned / NeedsOwnedConfirmation / Skipped / Failed
    pub outcome: String,
    pub detail: Option<String>,
}

fn outcome_name(outcome: PurchaseOutcome) -> &'static str {
    match outcome {
        PurchaseOutcome::Skipped => "Skipped",
        PurchaseOutcome::Purchased => "Purchased",
        PurchaseOutcome::AlreadyOwned => "AlreadyOwned",
        PurchaseOutcome::NeedsOwnedConfirmation => "NeedsOwnedConfirmation",
        PurchaseOutcome::Failed => "Failed",
    }
}

/// 购买里程碑日志（与 sync/queue 同路径写入缓冲；详细命令输出仍由
/// detailedIpatoolLog 开关控制）。
fn push_log(state: &AppState, level: &str, key: &str, args: &[&str]) {
    let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    state
        .log_buffer
        .push(crate::commands::LogEntryDto::new(
            level,
            crate::commands::JsMessage::key(key, &owned),
        ));
}

/// 购买（对齐 C# PurchaseService.PurchaseAsync 的前置策略）：
/// 已购跳过；非免费跳过（detail=NonFree）；模拟账户直通写库；
/// Purchased / AlreadyOwned / NeedsOwnedConfirmation 三种结果均写入已购记录。
#[tauri::command(async)]
pub fn purchase(
    state: State<'_, AppState>,
    bundle_id: String,
    price: String,
    purchased: String,
    platform: String,
) -> Result<PurchaseDto, String> {
    let lang = crate::i18n::Lang::from_state(&state);
    let bundle_id = bundle_id.trim().to_string();
    if bundle_id.is_empty() {
        return Ok(PurchaseDto {
            bundle_id,
            outcome: "Skipped".into(),
            detail: None,
        });
    }
    if crate::core::purchases::status_policy::is_purchased(Some(&purchased)) {
        return Ok(PurchaseDto {
            bundle_id,
            outcome: "Skipped".into(),
            detail: None,
        });
    }
    if !crate::core::purchases::status_policy::is_price_free_for_purchase(Some(&price)) {
        push_log(&state, "info", "Purchase/Log/SkippedNonFree", &[&bundle_id]);
        return Ok(PurchaseDto {
            bundle_id,
            outcome: "Skipped".into(),
            detail: Some("NonFree".into()),
        });
    }

    let (account, is_mock) = {
        let session = state.session.lock().unwrap();
        (
            session.account.clone().unwrap_or_default(),
            session.logged_in && session.is_mock,
        )
    };

    if is_mock {
        mark_purchased(&state, lang, &bundle_id, &account, &platform)?;
        push_log(&state, "success", "Purchase/Log/Success", &[&bundle_id]);
        return Ok(PurchaseDto {
            bundle_id,
            outcome: "Purchased".into(),
            detail: Some("Mock".into()),
        });
    }

    push_log(&state, "info", "Purchase/Log/Start", &[&bundle_id]);
    let exe_path = crate::resolver::resolve_executable_path(&state);
    let passphrase = crate::state::get_passphrase()
        .ok_or_else(|| lang.message("error-missing-passphrase"))?;
    let detailed_log = state.config.lock().unwrap().detailed_ipatool_log;

    let client = IpatoolClient::new(exe_path);
    let cancel = AtomicBool::new(false);
    let outcome_result = if detailed_log {
        let mut sink = |log: crate::core::purchases::sync_service::LogMessage| {
            state.log_buffer.push((&log).into());
        };
        client.purchase_app(&bundle_id, Some(&passphrase), &cancel, Some(&mut sink), &platform)
    } else {
        client.purchase_app(&bundle_id, Some(&passphrase), &cancel, None, &platform)
    };

    let result = outcome_result.map_err(|e| {
        lang.message_with("error-purchase-command-failed", &[("error", &format!("{e:?}"))])
    })?;

    let payload = result.output_or_error_raw();
    let payload_ref = if payload.trim().is_empty() {
        None
    } else {
        Some(payload.as_str())
    };
    let outcome = response_interpreter::interpret(payload_ref);

    if matches!(
        outcome,
        PurchaseOutcome::Purchased
            | PurchaseOutcome::AlreadyOwned
            | PurchaseOutcome::NeedsOwnedConfirmation
    ) {
        mark_purchased(&state, lang, &bundle_id, &account, &platform)?;
    }

    match outcome {
        PurchaseOutcome::Purchased => {
            push_log(&state, "success", "Purchase/Log/Success", &[&bundle_id]);
        }
        PurchaseOutcome::AlreadyOwned | PurchaseOutcome::NeedsOwnedConfirmation => {
            push_log(&state, "success", "Purchase/Log/AlreadyOwned", &[&bundle_id]);
        }
        PurchaseOutcome::Failed => {
            push_log(&state, "error", "Purchase/Log/Failed", &[&bundle_id]);
        }
        PurchaseOutcome::Skipped => {}
    }

    Ok(PurchaseDto {
        bundle_id,
        outcome: outcome_name(outcome).into(),
        detail: Some(payload),
    })
}

fn mark_purchased(
    state: &AppState,
    lang: crate::i18n::Lang,
    bundle_id: &str,
    account: &str,
    platform: &str,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    let db = db.as_ref().ok_or_else(|| lang.message("error-db-not-initialized"))?;
    db.save_purchased_app(bundle_id, account, Some("purchased"), platform)
        .map_err(|e| e.to_string())
}

/// 三点菜单：标记为已购买 / 未购买。
#[tauri::command]
pub fn purchases_mark(
    state: State<'_, AppState>,
    bundle_id: String,
    status: String,
    platform: String,
) -> Result<(), String> {
    let lang = crate::i18n::Lang::from_state(&state);
    let account = state
        .session
        .lock()
        .unwrap()
        .account
        .clone()
        .ok_or_else(|| lang.message("error-not-signed-in"))?;
    let db = state.db.lock().unwrap();
    let db = db.as_ref().ok_or_else(|| lang.message("error-db-not-initialized"))?;
    db.save_purchased_app(&bundle_id, &account, Some(status.as_str()), &platform)
        .map_err(|e| e.to_string())
}

/// 三点菜单：移除标记。
#[tauri::command]
pub fn purchases_unmark(
    state: State<'_, AppState>,
    bundle_id: String,
    platform: String,
) -> Result<(), String> {
    let lang = crate::i18n::Lang::from_state(&state);
    let account = state
        .session
        .lock()
        .unwrap()
        .account
        .clone()
        .ok_or_else(|| lang.message("error-not-signed-in"))?;
    let db = state.db.lock().unwrap();
    let db = db.as_ref().ok_or_else(|| lang.message("error-db-not-initialized"))?;
    db.remove_purchased_app(&bundle_id, &account, &platform)
        .map_err(|e| e.to_string())
}

/// 本地已购记录总数（全部账户；设置页清空确认对话框显示）。
#[tauri::command]
pub fn purchases_total_count(state: State<'_, AppState>) -> Result<i64, String> {
    let lang = crate::i18n::Lang::from_state(&state);
    let db = state.db.lock().unwrap();
    let db = db.as_ref().ok_or_else(|| lang.message("error-db-not-initialized"))?;
    db.get_total_count(None).map_err(|e| e.to_string())
}

/// 清空本地已购记录（全部账户，不可恢复；设置页"清空本地购买记录"卡片）。
/// 返回（清空前，清空后）条数，供成功提示展示。
#[tauri::command]
pub fn purchases_clear(state: State<'_, AppState>) -> Result<(i64, i64), String> {
    let lang = crate::i18n::Lang::from_state(&state);
    let db = state.db.lock().unwrap();
    let db = db.as_ref().ok_or_else(|| lang.message("error-db-not-initialized"))?;
    let before = db.get_total_count(None).map_err(|e| e.to_string())?;
    db.clear_purchased_apps(None).map_err(|e| e.to_string())?;
    let after = db.get_total_count(None).map_err(|e| e.to_string())?;
    Ok((before, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_name_covers_all_outcomes() {
        assert_eq!(outcome_name(PurchaseOutcome::Skipped), "Skipped");
        assert_eq!(outcome_name(PurchaseOutcome::Purchased), "Purchased");
        assert_eq!(outcome_name(PurchaseOutcome::AlreadyOwned), "AlreadyOwned");
        assert_eq!(
            outcome_name(PurchaseOutcome::NeedsOwnedConfirmation),
            "NeedsOwnedConfirmation"
        );
        assert_eq!(outcome_name(PurchaseOutcome::Failed), "Failed");
    }
}
