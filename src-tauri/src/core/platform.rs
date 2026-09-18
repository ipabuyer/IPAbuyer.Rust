//! 平台标识：App Store 平台维度贯穿搜索、购买状态、下载与已购同步。
//!
//! 当前支持 `ios`（缺省，iTunes Search `entity=software`、ipatool 缺省行为）、
//! `ipad`（`entity=iPadSoftware`、ipatool `--platform ipad`）与 `macos`
//! （`entity=macSoftware`、ipatool `--platform macos`），后两者需 ipatool
//! ≥ 2.6.0。tvOS/visionOS 因公开搜索 API 无数据源暂不引入。

/// iOS App Store（缺省平台）。
pub const IOS: &str = "ios";
/// iPadOS（iPad App Store 条目，含 iPad 独占 App）。
pub const IPAD: &str = "ipad";
/// Mac App Store。
pub const MACOS: &str = "macos";

/// 归一化平台标识：无法识别的一律归一为 `ios`（缺省平台，向前兼容旧数据）。
pub fn normalize(platform: &str) -> &'static str {
    let trimmed = platform.trim();
    if trimmed.eq_ignore_ascii_case(MACOS) {
        MACOS
    } else if trimmed.eq_ignore_ascii_case(IPAD) {
        IPAD
    } else {
        IOS
    }
}

/// ipatool 命令的 `--platform` 取值；`ios` 为 ipatool 缺省行为不传参，
/// 兼容自定义 ipatool 2.5.0（无该参数）。
pub fn ipatool_arg(platform: &str) -> Option<&'static str> {
    match normalize(platform) {
        IPAD => Some("ipad"),
        MACOS => Some("macos"),
        _ => None,
    }
}

/// 平台对应的商店显示名（品牌名，各语言通用；用于日志参数）。
pub fn display_name(platform: &str) -> &'static str {
    match normalize(platform) {
        IPAD => "iPad App Store",
        MACOS => "Mac App Store",
        _ => "App Store",
    }
}

/// 已购记录/查找的组合键：同一 bundleId 在不同商店是不同条目。
/// bundleId 统一小写——数据库按小写归一存储（trim + lowercase），
/// 而 iTunes 搜索返回的 bundleId 大小写不定（如 com.apple.TestFlight）。
pub fn purchase_key(platform: &str, bundle_id: &str) -> String {
    format!(
        "{}:{}",
        normalize(platform),
        bundle_id.trim().to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_maps_unknown_to_ios() {
        assert_eq!(normalize("ios"), IOS);
        assert_eq!(normalize(" ipad "), IPAD);
        assert_eq!(normalize("MACOS"), MACOS);
        assert_eq!(normalize(""), IOS);
        assert_eq!(normalize("tvos"), IOS);
    }

    #[test]
    fn ipatool_arg_set_for_non_ios_platforms() {
        assert_eq!(ipatool_arg(IOS), None);
        assert_eq!(ipatool_arg(IPAD), Some("ipad"));
        assert_eq!(ipatool_arg(MACOS), Some("macos"));
        assert_eq!(ipatool_arg("junk"), None);
    }

    #[test]
    fn display_name_covers_all_platforms() {
        assert_eq!(display_name(IOS), "App Store");
        assert_eq!(display_name(IPAD), "iPad App Store");
        assert_eq!(display_name(MACOS), "Mac App Store");
    }

    #[test]
    fn purchase_key_distinguishes_platforms_and_normalizes_case() {
        assert_eq!(purchase_key(IOS, "com.a"), "ios:com.a");
        assert_eq!(purchase_key("ipad", " com.a "), "ipad:com.a");
        assert_ne!(purchase_key(IOS, "com.a"), purchase_key(MACOS, "com.a"));
        assert_ne!(purchase_key(IOS, "com.a"), purchase_key(IPAD, "com.a"));
        // DB 存储小写归一，搜索返回的 bundleId 大小写不定，必须按小写匹配
        assert_eq!(
            purchase_key(IOS, "com.apple.TestFlight"),
            purchase_key(IOS, "com.apple.testflight")
        );
    }
}
