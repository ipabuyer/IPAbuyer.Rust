//! App Catalog 服务：国家/地区归一与搜索编排。
//!
//! 移植自主仓库 `AppCatalogService`。与 C# 版的差异：已购买记录经参数
//! 传入（宿主从数据库读取），国家/地区合法性经 `is_valid_country` 回调
//! 校验（storefront 目录由宿主提供，阶段 3 评估移植数据表）。

use std::collections::HashMap;

use crate::core::appcatalog::search_client;
use crate::core::appcatalog::search_parser::{self, SearchResult};

/// 国家/地区代码默认值。
pub const DEFAULT_COUNTRY_CODE: &str = "cn";

/// 归一化国家/地区代码：裁剪、小写，非法值回退默认 `cn`。
pub fn normalize_country_code(
    code: Option<&str>,
    is_valid_country: &dyn Fn(&str) -> bool,
) -> String {
    match code {
        None => DEFAULT_COUNTRY_CODE.to_string(),
        Some(code) if code.trim().is_empty() => DEFAULT_COUNTRY_CODE.to_string(),
        Some(code) => {
            let normalized = code.trim().to_lowercase();
            if is_valid_country(&normalized) {
                normalized
            } else {
                DEFAULT_COUNTRY_CODE.to_string()
            }
        }
    }
}

/// 搜索目录：请求 iTunes Search 并解析为带购买状态的结果列表。
/// 同时检索 iOS（`entity=software`）与 Mac（`entity=macSoftware`）两个
/// App Store，iOS 结果在前；两侧都超时/为空时返回 `None`。
pub fn search_catalog(
    app_name: &str,
    limit: i64,
    country_code: &str,
    is_valid_country: &dyn Fn(&str) -> bool,
    purchased_apps: &HashMap<String, String>,
) -> Option<Vec<SearchResult>> {
    let country = normalize_country_code(Some(country_code), is_valid_country);
    let ios = search_one(
        app_name,
        limit,
        &country,
        search_client::ENTITY_SOFTWARE,
        crate::core::platform::IOS,
        purchased_apps,
    );
    let macos = search_one(
        app_name,
        limit,
        &country,
        search_client::ENTITY_MAC_SOFTWARE,
        crate::core::platform::MACOS,
        purchased_apps,
    );

    match (ios, macos) {
        (None, None) => None,
        (ios, macos) => Some(merge_results(
            ios.unwrap_or_default(),
            macos.unwrap_or_default(),
        )),
    }
}

/// 检索单一实体并解析为带购买状态的结果；超时或响应为空返回 `None`。
fn search_one(
    app_name: &str,
    limit: i64,
    country_code: &str,
    entity: &str,
    platform: &str,
    purchased_apps: &HashMap<String, String>,
) -> Option<Vec<SearchResult>> {
    let response = search_client::search(app_name, limit, country_code, entity);
    if response.timed_out {
        return None;
    }

    let payload = response.output_or_error_raw();
    if payload.trim().is_empty() {
        return None;
    }

    search_parser::parse(&payload, platform, purchased_apps)
}

/// 合并两个平台的结果：iOS 在前、macOS 在后（同类内保持 API 返回顺序）。
fn merge_results(ios: Vec<SearchResult>, macos: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut merged = ios;
    merged.extend(macos);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_country_code_falls_back_to_default() {
        assert_eq!(normalize_country_code(None, &|_| true), "cn");
        assert_eq!(normalize_country_code(Some("  "), &|_| true), "cn");
        assert_eq!(normalize_country_code(Some(" US "), &|_| true), "us");
        assert_eq!(
            normalize_country_code(Some("xx"), &|code| code == "us"),
            "cn"
        );
    }

    #[test]
    fn merge_results_puts_ios_before_macos() {
        let item = |bundle_id: &str, platform: &str| SearchResult {
            bundle_id: bundle_id.to_string(),
            id: None,
            name: None,
            developer: None,
            artwork_url: None,
            price: "free".to_string(),
            version: None,
            platform: platform.to_string(),
            purchased: "not_purchased".to_string(),
        };

        let merged = merge_results(
            vec![item("com.ios", crate::core::platform::IOS)],
            vec![item("com.mac", crate::core::platform::MACOS)],
        );

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].bundle_id, "com.ios");
        assert_eq!(merged[1].bundle_id, "com.mac");
        assert_eq!(merged[1].platform, "macos");
    }
}
