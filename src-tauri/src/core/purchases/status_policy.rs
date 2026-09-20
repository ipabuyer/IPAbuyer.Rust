//! 购买状态策略。
//!
//! 移植自主仓库 `PurchaseStatusPolicy` 的纯逻辑部分。与 C# 版的关键差异：
//! C# 版状态值是本地化显示字符串，本模块按本地化原则（references/localization.md）
//! 改用规范 token（`purchased` / `not_purchased` / `blocked`），宿主负责映射为
//! `.resw` 显示文案；价格中的本地化"免费"文案同样由宿主归一后传入。

use std::collections::HashMap;

use crate::core::purchases::record_status;

/// 已购买（数据库存储与对外的规范 token）。
pub const STATUS_PURCHASED: &str = "purchased";
/// 未购买（免费或价格未知，可执行购买）。
pub const STATUS_NOT_PURCHASED: &str = "not_purchased";
/// 无法购买（非免费或当前不可购买；不入库，由价格推导）。
pub const STATUS_PURCHASE_BLOCKED: &str = "blocked";

/// 数据库/历史状态归一：可识别（含历史 owned）即已购买，否则未购买。
pub fn normalize_stored_status(status: Option<&str>) -> &'static str {
    if record_status::try_normalize(status).is_some() {
        STATUS_PURCHASED
    } else {
        STATUS_NOT_PURCHASED
    }
}

/// 状态是否为已购买。
pub fn is_purchased(status: Option<&str>) -> bool {
    normalize_stored_status(status) == STATUS_PURCHASED
}

/// 状态是否为无法购买（精确匹配 blocked token）。
pub fn is_purchase_blocked(status: Option<&str>) -> bool {
    matches_status(status, STATUS_PURCHASE_BLOCKED)
}

/// 状态是否可归入"未购买"筛选（未购买、无法购买与空值都算：它们都尚未购买）。
pub fn is_can_purchase(status: Option<&str>) -> bool {
    match status {
        None => true,
        Some(value) => {
            let trimmed = value.trim();
            trimmed.is_empty()
                || matches_status(Some(trimmed), STATUS_PURCHASE_BLOCKED)
                || matches_status(Some(trimmed), STATUS_NOT_PURCHASED)
        }
    }
}

/// 已购查找的组合键：平台与 bundleId 共同确定一条商店条目。
pub fn purchase_key(platform: &str, bundle_id: &str) -> String {
    crate::core::platform::purchase_key(platform, bundle_id)
}

/// 根据数据库记录推导搜索结果状态；无记录时按价格推导。
/// `purchased_apps` 的键为 [`purchase_key`] 组合键。
pub fn resolve_search_status(
    platform: Option<&str>,
    bundle_id: Option<&str>,
    price: Option<&str>,
    purchased_apps: &HashMap<String, String>,
) -> &'static str {
    if let Some(bundle_id) = bundle_id {
        if !bundle_id.trim().is_empty() {
            let key = purchase_key(platform.unwrap_or(crate::core::platform::IOS), bundle_id);
            if let Some(status) = purchased_apps.get(&key) {
                return normalize_stored_status(Some(status));
            }
        }
    }

    resolve_unpurchased_status(price)
}

/// 无数据库记录时的价格推导：免费或价格未知为未购买，否则无法购买。
pub fn resolve_unpurchased_status(price: Option<&str>) -> &'static str {
    match price {
        Some(value) if !value.trim().is_empty() => {
            if is_price_free_for_purchase(Some(value)) {
                STATUS_NOT_PURCHASED
            } else {
                STATUS_PURCHASE_BLOCKED
            }
        }
        _ => STATUS_NOT_PURCHASED,
    }
}

/// 判断价格是否可免费购买：数字 ≤ 0 或字面 `free`（大小写不敏感）。
/// 本地化"免费"文案由宿主归一为 `free` 后传入。
pub fn is_price_free_for_purchase(price: Option<&str>) -> bool {
    let Some(value) = price else {
        return false;
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    trimmed
        .parse::<f64>()
        .map(|number| number <= 0.0)
        .unwrap_or_else(|_| trimmed.eq_ignore_ascii_case("free"))
}

/// 价格显示归一：非正数与 `free` 归一为字面 `free`（宿主映射为本地化文案），其余原样返回。
pub fn normalize_price_for_display(price: Option<&str>) -> String {
    let Some(value) = price else {
        return String::new();
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.parse::<f64>().map(|number| number <= 0.0) == Ok(true)
        || trimmed.eq_ignore_ascii_case("free")
    {
        return "free".to_string();
    }

    trimmed.to_string()
}

fn matches_status(status: Option<&str>, expected: &str) -> bool {
    status.is_some_and(|actual| actual.trim().eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn purchased_apps(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(app_id, status)| (app_id.to_string(), status.to_string()))
            .collect()
    }

    #[test]
    fn normalize_stored_status_merges_owned_into_purchased() {
        assert_eq!(normalize_stored_status(Some("purchased")), STATUS_PURCHASED);
        assert_eq!(normalize_stored_status(Some("owned")), STATUS_PURCHASED);
        assert_eq!(normalize_stored_status(Some("已拥有")), STATUS_PURCHASED);
        assert_eq!(normalize_stored_status(Some("junk")), STATUS_NOT_PURCHASED);
        assert_eq!(normalize_stored_status(None), STATUS_NOT_PURCHASED);
    }

    #[test]
    fn is_can_purchase_includes_empty_blocked_and_not_purchased() {
        assert!(is_can_purchase(None));
        assert!(is_can_purchase(Some("  ")));
        assert!(is_can_purchase(Some("not_purchased")));
        assert!(is_can_purchase(Some("blocked")));
        assert!(!is_can_purchase(Some("purchased")));
        assert!(!is_can_purchase(Some("junk")));
    }

    #[test]
    fn resolve_search_status_prefers_database_records() {
        let records = purchased_apps(&[
            (purchase_key("ios", "com.a").as_str(), "owned"),
            (purchase_key("ios", "com.b").as_str(), "purchased"),
        ]);

        assert_eq!(
            resolve_search_status(Some("ios"), Some("com.a"), Some("0"), &records),
            STATUS_PURCHASED
        );
        assert_eq!(
            resolve_search_status(Some("ios"), Some("com.b"), Some("6.00"), &records),
            STATUS_PURCHASED
        );
        assert_eq!(
            resolve_search_status(Some("ios"), Some("com.missing"), Some("6.00"), &records),
            STATUS_PURCHASE_BLOCKED
        );
    }

    #[test]
    fn resolve_search_status_is_platform_scoped() {
        // 同一 bundleId 在两个商店是不同条目：仅 Mac 记录不应污染 iOS 条目。
        let records = purchased_apps(&[(purchase_key("macos", "com.a").as_str(), "purchased")]);

        assert_eq!(
            resolve_search_status(Some("macos"), Some("com.a"), Some("0"), &records),
            STATUS_PURCHASED
        );
        assert_eq!(
            resolve_search_status(Some("ios"), Some("com.a"), Some("0"), &records),
            STATUS_NOT_PURCHASED
        );
    }

    #[test]
    fn resolve_unpurchased_status_derives_from_price() {
        assert_eq!(resolve_unpurchased_status(None), STATUS_NOT_PURCHASED);
        assert_eq!(resolve_unpurchased_status(Some("  ")), STATUS_NOT_PURCHASED);
        assert_eq!(resolve_unpurchased_status(Some("0")), STATUS_NOT_PURCHASED);
        assert_eq!(
            resolve_unpurchased_status(Some("free")),
            STATUS_NOT_PURCHASED
        );
        assert_eq!(
            resolve_unpurchased_status(Some("6.00")),
            STATUS_PURCHASE_BLOCKED
        );
    }

    #[test]
    fn is_price_free_for_purchase_accepts_numbers_and_free_literal() {
        assert!(!is_price_free_for_purchase(None));
        assert!(!is_price_free_for_purchase(Some("  ")));
        assert!(is_price_free_for_purchase(Some("0")));
        assert!(is_price_free_for_purchase(Some("-1")));
        assert!(is_price_free_for_purchase(Some("0.00")));
        assert!(is_price_free_for_purchase(Some("Free")));
        assert!(!is_price_free_for_purchase(Some("6.00")));
    }

    #[test]
    fn normalize_price_for_display_maps_non_positive_to_free() {
        assert_eq!(normalize_price_for_display(None), "");
        assert_eq!(normalize_price_for_display(Some("  ")), "");
        assert_eq!(normalize_price_for_display(Some("0")), "free");
        assert_eq!(normalize_price_for_display(Some("FREE")), "free");
        assert_eq!(normalize_price_for_display(Some(" 6.00 ")), "6.00");
    }
}
