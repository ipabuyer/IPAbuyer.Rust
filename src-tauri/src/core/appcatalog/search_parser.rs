//! iTunes Search 响应解析。
//!
//! 移植自主仓库 `AppStoreSearchResponseParser`。属性读取与 C# 的
//! `JsonElement.TryGetProperty` 一致为大小写敏感；无效 JSON 返回 `None`
//! （C# 版解析异常由上层捕获为 null，此处收敛到同一契约）。

use std::collections::HashMap;

use serde_json::Value;

use crate::core::purchases::status_policy;

/// 搜索结果条目（字段与主应用 `SearchResult` 对应；`price`/`purchased` 为
/// Core 归一后的规范值，宿主负责本地化显示）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SearchResult {
    pub bundle_id: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub developer: Option<String>,
    pub artwork_url: Option<String>,
    pub price: String,
    pub version: Option<String>,
    pub purchased: String,
}

/// 解析搜索响应；payload 非对象或缺 `results` 数组时返回 `None`。
pub fn parse(payload: &str, purchased_apps: &HashMap<String, String>) -> Option<Vec<SearchResult>> {
    let root: Value = serde_json::from_str(payload).ok()?;
    let results = root.get("results")?.as_array()?;

    let mut items = Vec::new();
    for app in results {
        let bundle_id = get_bundle_id(app).unwrap_or_default();
        let price = status_policy::normalize_price_for_display(get_price_value(app).as_deref());
        items.push(SearchResult {
            bundle_id: bundle_id.clone(),
            id: get_property(app, "trackId"),
            name: get_property(app, "trackName"),
            developer: get_property(app, "sellerName"),
            artwork_url: get_property(app, "artworkUrl100"),
            price: price.clone(),
            version: get_property(app, "version"),
            purchased: status_policy::resolve_search_status(
                Some(bundle_id.as_str()),
                Some(price.as_str()),
                purchased_apps,
            )
            .to_string(),
        });
    }

    Some(items)
}

fn get_bundle_id(element: &Value) -> Option<String> {
    get_property(element, "bundleID").or_else(|| get_property(element, "bundleId"))
}

fn get_property(element: &Value, name: &str) -> Option<String> {
    element.get(name).and_then(read_scalar)
}

fn get_price_value(element: &Value) -> Option<String> {
    let price = element.get("price")?;
    match price {
        Value::Number(number) => number
            .as_f64()
            .map(|value| format!("{value:.2}"))
            .or_else(|| read_scalar(price)),
        other => read_scalar(other),
    }
}

fn read_scalar(element: &Value) -> Option<String> {
    match element {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        other => Some(other.to_string()),
    }
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
    fn parse_maps_results_and_resolves_purchased_status() {
        let payload = r#"{"resultCount":2,"results":[
{"trackId":1,"bundleID":"com.owned","trackName":"Owned App","sellerName":"Dev A","artworkUrl100":"https://a/1.png","price":0,"version":"1.0"},
{"trackId":2,"bundleId":"com.paid","trackName":"Paid App","sellerName":"Dev B","artworkUrl100":"https://a/2.png","price":6.0,"version":"2.1"}]}"#;

        let records = purchased_apps(&[("com.owned", "owned")]);
        let items = parse(payload, &records).expect("valid payload");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].bundle_id, "com.owned");
        assert_eq!(items[0].id.as_deref(), Some("1"));
        assert_eq!(items[0].price, "free");
        assert_eq!(items[0].purchased, "purchased");
        assert_eq!(items[1].bundle_id, "com.paid");
        assert_eq!(items[1].price, "6.00");
        assert_eq!(items[1].purchased, "blocked");
    }

    #[test]
    fn parse_rejects_invalid_payloads() {
        let records = purchased_apps(&[]);

        assert!(parse("not json", &records).is_none());
        assert!(parse("{\"results\":\"not an array\"}", &records).is_none());
        assert!(parse("{\"resultCount\":0}", &records).is_none());
    }

    #[test]
    fn parse_keeps_entries_without_bundle_id_like_csharp() {
        // 对齐 C#：解析器不过滤无 bundleId 的条目（由上层处理）。
        let payload = r#"{"results":[{"trackId":3,"trackName":"No Bundle"},{"trackId":4,"bundleId":"com.ok","price":0}]}"#;

        let items = parse(payload, &purchased_apps(&[])).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].bundle_id, "");
        assert_eq!(items[1].bundle_id, "com.ok");
    }
}
