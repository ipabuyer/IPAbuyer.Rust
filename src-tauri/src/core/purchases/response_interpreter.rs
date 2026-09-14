//! 购买响应解释。
//!
//! 移植自主仓库 `PurchaseResponseInterpreter`，判定优先级与触发词完全一致：
//! `alreadyOwned` > `success` > STDQ 报错 > 失败。

/// 购买命令结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseOutcome {
    /// 未执行（参数缺失、已购买、非免费）。
    Skipped,
    /// 购买成功。
    Purchased,
    /// 账户已拥有该 App。
    AlreadyOwned,
    /// STDQ 报错，疑似已拥有。
    NeedsOwnedConfirmation,
    /// 失败。
    Failed,
}

/// 解释 purchase 命令的 JSON 输出（允许夹带日志噪声）。
pub fn interpret(payload: Option<&str>) -> PurchaseOutcome {
    let Some(payload) = payload else {
        return PurchaseOutcome::Failed;
    };
    if payload.trim().is_empty() {
        return PurchaseOutcome::Failed;
    }

    let tokens = crate::core::json::enumerate_tokens(Some(payload));
    let lowercase = payload.to_ascii_lowercase();

    if lowercase.contains("\"alreadyowned\":true")
        || tokens
            .iter()
            .any(|token| crate::core::json::try_read_boolean(token, "alreadyOwned") == Some(true))
    {
        return PurchaseOutcome::AlreadyOwned;
    }

    if lowercase.contains("\"success\":true")
        || tokens
            .iter()
            .any(|token| crate::core::json::try_read_boolean(token, "success") == Some(true))
    {
        return PurchaseOutcome::Purchased;
    }

    if lowercase.contains("failed to purchase item with param 'stdq'") {
        PurchaseOutcome::NeedsOwnedConfirmation
    } else {
        PurchaseOutcome::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpret_unsuccessful_payload_returns_failed() {
        for payload in [
            None,
            Some(""),
            Some("  "),
            Some("{\"success\":false}"),
            Some("unrecognized failure"),
        ] {
            assert_eq!(interpret(payload), PurchaseOutcome::Failed);
        }
    }

    #[test]
    fn interpret_success_payload_returns_purchased() {
        for payload in [
            "{\"success\":true}",
            "{\"success\":\"true\"}",
            "{\"success\":1}",
            "debug\n{\"success\":true}",
        ] {
            assert_eq!(interpret(Some(payload)), PurchaseOutcome::Purchased);
        }
    }

    #[test]
    fn interpret_already_owned_takes_precedence_over_success() {
        assert_eq!(
            interpret(Some("{\"alreadyOwned\":true,\"success\":true}")),
            PurchaseOutcome::AlreadyOwned
        );
    }

    #[test]
    fn interpret_stdq_failure_requires_owned_confirmation() {
        for payload in [
            "failed to purchase item with param 'STDQ'",
            "FAILED TO PURCHASE ITEM WITH PARAM 'stdq'",
        ] {
            assert_eq!(
                interpret(Some(payload)),
                PurchaseOutcome::NeedsOwnedConfirmation
            );
        }
    }

    #[test]
    fn interpret_already_owned_detected_in_embedded_json() {
        assert_eq!(
            interpret(Some("debug\n{\"alreadyOwned\":true}")),
            PurchaseOutcome::AlreadyOwned
        );
    }

    #[test]
    fn interpret_already_owned_reads_string_values_through_json_tokens() {
        assert_eq!(
            interpret(Some("{\"alreadyOwned\": \"true\"}")),
            PurchaseOutcome::AlreadyOwned
        );
    }

    #[test]
    fn interpret_false_flags_do_not_trigger_owned_or_purchased() {
        assert_eq!(
            interpret(Some("{\"alreadyOwned\":false,\"success\":false}")),
            PurchaseOutcome::Failed
        );
    }
}
