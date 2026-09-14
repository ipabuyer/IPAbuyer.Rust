//! `list-purchases` 单页输出解析。
//!
//! 移植自主仓库 `OwnedAppsPageParser`。成功形如
//! `{"level":"info","count":5,"totalCount":1753,"page":1,"apps":[{"bundleID":"..."},...]}`；
//! 失败形如 `{"level":"error","error":"...","success":false}`。

use crate::core::json;

/// 单页解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedAppsPage {
    pub success: bool,
    pub total_count: i64,
    pub bundle_ids: Vec<String>,
    pub error_message: Option<String>,
}

/// 解析一页输出；无法识别时视为失败并保留原始 payload 作为错误信息。
pub fn parse(payload: Option<&str>) -> OwnedAppsPage {
    let Some(payload) = payload else {
        return failure(None);
    };
    if payload.trim().is_empty() {
        return failure(Some(payload.to_string()));
    }

    for token in json::enumerate_tokens(Some(payload)) {
        if is_error_token(&token) {
            let error = json::try_read_string(&token, &["error", "message"])
                .unwrap_or_else(|| payload.to_string());
            return failure(Some(error));
        }

        if !is_success_token(&token) {
            continue;
        }

        let total_count = read_int(&token, "totalCount");
        let bundle_ids = read_bundle_ids(&token);
        return OwnedAppsPage {
            success: true,
            total_count,
            bundle_ids,
            error_message: None,
        };
    }

    failure(Some(payload.to_string()))
}

fn failure(error_message: Option<String>) -> OwnedAppsPage {
    OwnedAppsPage {
        success: false,
        total_count: 0,
        bundle_ids: Vec::new(),
        error_message,
    }
}

fn is_error_token(token: &json::Value) -> bool {
    if let Some(level) = json::try_read_string(token, &["level"]) {
        return level.eq_ignore_ascii_case("error");
    }

    json::try_read_boolean(token, "success") == Some(false)
}

fn is_success_token(token: &json::Value) -> bool {
    if !token.is_object() {
        return false;
    }

    if let Some(level) = json::try_read_string(token, &["level"]) {
        return level.eq_ignore_ascii_case("info") && has_apps_array(token);
    }

    has_apps_array(token)
}

fn has_apps_array(token: &json::Value) -> bool {
    json::try_get_property(token, "apps").is_some_and(json::Value::is_array)
}

fn read_int(token: &json::Value, name: &str) -> i64 {
    let Some(child) = json::try_get_property(token, name) else {
        return 0;
    };

    match child {
        json::Value::Number(number) => number.as_i64().unwrap_or(0),
        other => json::read_scalar_as_string(other)
            .and_then(|text| text.trim().parse::<i64>().ok())
            .unwrap_or(0),
    }
}

fn read_bundle_ids(token: &json::Value) -> Vec<String> {
    let Some(apps) = json::try_get_property(token, "apps").and_then(|apps| apps.as_array()) else {
        return Vec::new();
    };

    apps.iter()
        .filter_map(|app| json::try_read_string(app, &["bundleID", "bundleId"]))
        .filter(|bundle_id| !bundle_id.trim().is_empty())
        .map(|bundle_id| bundle_id.trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_success_payload_returns_bundle_ids_and_total_count() {
        let payload = r#"{"level":"info","count":2,"totalCount":1753,"page":1,"apps":[
{"id":1487448370,"bundleID":"com.ingka.ikea.app.cn.prod","name":"IKEA 宜家家居","version":"5.19.0","price":0,"purchaseDate":"2026-09-11T11:59:20Z"},
{"id":342576766,"bundleID":"com.amazon.AmazonCN","name":"亚马逊购物","version":"26.18","price":0,"purchaseDate":"2026-09-11T11:59:03Z"}],
"time":"2026-09-12T10:24:26+08:00"}"#;

        let page = parse(Some(payload));

        assert!(page.success);
        assert_eq!(page.total_count, 1753);
        assert_eq!(page.bundle_ids.len(), 2);
        assert!(
            page.bundle_ids
                .contains(&"com.ingka.ikea.app.cn.prod".to_string())
        );
        assert!(page.bundle_ids.contains(&"com.amazon.AmazonCN".to_string()));
    }

    #[test]
    fn parse_empty_apps_payload_returns_success_with_no_bundle_ids() {
        let payload = r#"{"level":"info","count":0,"totalCount":1753,"page":999,"apps":[],"time":"2026-09-12T10:25:31+08:00"}"#;

        let page = parse(Some(payload));

        assert!(page.success);
        assert_eq!(page.total_count, 1753);
        assert!(page.bundle_ids.is_empty());
    }

    #[test]
    fn parse_error_payload_returns_failure_with_message() {
        let payload = r#"{"level":"error","error":"max results must not exceed 100","success":false,"time":"2026-09-12T10:24:49+08:00"}"#;

        let page = parse(Some(payload));

        assert!(!page.success);
        assert_eq!(
            page.error_message.as_deref(),
            Some("max results must not exceed 100")
        );
        assert!(page.bundle_ids.is_empty());
    }

    #[test]
    fn parse_wrong_passphrase_error_returns_failure_with_underlying_message() {
        let payload = r#"{"level":"error","error":"failed to get account: failed to get item: aes.KeyUnwrap(): integrity check failed.","success":false,"time":"2026-09-12T10:24:49+08:00"}"#;

        let page = parse(Some(payload));

        assert!(!page.success);
        assert!(page.error_message.unwrap().contains("KeyUnwrap"));
    }

    #[test]
    fn parse_invalid_payload_returns_failure() {
        for payload in [None, Some(""), Some("  "), Some("not json at all")] {
            let page = parse(payload);
            assert!(!page.success);
            assert!(page.bundle_ids.is_empty());
        }
    }
}
