//! 平台标识：App Store 平台维度贯穿搜索、购买状态、下载与已购同步。
//!
//! 当前支持 `ios`（默认，对应 iTunes Search `entity=software` 与 ipatool
//! 缺省行为）与 `macos`（`entity=macSoftware`、ipatool `--platform macos`，
//! 需 ipatool ≥ 2.6.0）。

/// iOS App Store（缺省平台）。
pub const IOS: &str = "ios";
/// Mac App Store。
pub const MACOS: &str = "macos";

/// 归一化平台标识：非 `macos` 的一律归一为 `ios`（缺省平台，向前兼容旧数据）。
pub fn normalize(platform: &str) -> &'static str {
    if platform.trim().eq_ignore_ascii_case(MACOS) {
        MACOS
    } else {
        IOS
    }
}

/// ipatool 命令的 `--platform` 取值；`ios` 为 ipatool 缺省行为不传参，
/// 兼容自定义 ipatool 2.5.0（无该参数）。
pub fn ipatool_arg(platform: &str) -> Option<&'static str> {
    match normalize(platform) {
        MACOS => Some("macos"),
        _ => None,
    }
}

/// 已购记录/查找的组合键：同一 bundleId 在两个商店是不同条目。
pub fn purchase_key(platform: &str, bundle_id: &str) -> String {
    format!("{}:{}", normalize(platform), bundle_id.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_maps_unknown_to_ios() {
        assert_eq!(normalize("ios"), IOS);
        assert_eq!(normalize(" macOS "), MACOS);
        assert_eq!(normalize(""), IOS);
        assert_eq!(normalize("tvos"), IOS);
    }

    #[test]
    fn ipatool_arg_only_set_for_macos() {
        assert_eq!(ipatool_arg(IOS), None);
        assert_eq!(ipatool_arg(MACOS), Some("macos"));
        assert_eq!(ipatool_arg("junk"), None);
    }

    #[test]
    fn purchase_key_distinguishes_platforms() {
        assert_eq!(purchase_key(IOS, "com.a"), "ios:com.a");
        assert_eq!(purchase_key("macos", " com.a "), "macos:com.a");
        assert_ne!(purchase_key(IOS, "com.a"), purchase_key(MACOS, "com.a"));
    }
}
