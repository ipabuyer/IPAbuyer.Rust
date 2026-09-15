//! `JsonPayload` 的 Rust 移植：宽容解析 ipatool 输出中的 JSON 片段。
//!
//! 移植自主仓库 `IPAbuyer.Core/Serialization/JsonPayload.cs`，行为逐条对齐：
//! - 首选整体解析（去首尾空白后以 `{`/`[` 开头）；
//! - 否则按行拆分（`}{` 视为换行），逐行解析；
//! - 行内解析失败时从第一个 `{`（其次 `[`）截取候选再试；
//! - 属性名匹配大小写不敏感；标量统一以字符串读取（数字/布尔转原文）。

pub use serde_json::Value;

/// 解析 payload 中能识别的全部 JSON 片段（对象或数组）。
pub fn enumerate_tokens(payload: Option<&str>) -> Vec<Value> {
    let Some(payload) = payload else {
        return Vec::new();
    };
    if payload.trim().is_empty() {
        return Vec::new();
    }

    let trimmed = payload.trim();
    let mut tokens = Vec::new();
    if looks_like_json(trimmed) {
        if let Some(token) = try_parse_token(trimmed) {
            tokens.push(token);
            return tokens;
        }
    }

    let normalized = payload.replace("}{", "}\n{");
    let lines: Vec<&str> = normalized
        .split(['\n', '\r'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    for line in &lines {
        if let Some(token) = try_parse_token(line) {
            tokens.push(token);
            continue;
        }

        if let Some(candidate) = find_embedded_json_candidate(line) {
            if let Some(token) = try_parse_token(&candidate) {
                tokens.push(token);
            }
        }
    }

    if lines.is_empty() && looks_like_json(trimmed) {
        if let Some(token) = try_parse_token(trimmed) {
            tokens.push(token);
        }
    }

    tokens
}

/// 尝试将文本解析为单个 JSON 值。
pub fn try_parse_token(json: &str) -> Option<Value> {
    if json.trim().is_empty() {
        return None;
    }
    serde_json::from_str(json).ok()
}

/// 读取布尔属性：接受布尔、布尔字符串（大小写不敏感）与数字（非零为真）；缺失/空为 `None`。
pub fn try_read_boolean(token: &Value, name: &str) -> Option<bool> {
    let child = try_get_property(token, name)?;
    match child {
        Value::Bool(value) => Some(*value),
        Value::String(text) => match text.to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => text.trim().parse::<f64>().ok().map(|number| number != 0.0),
        },
        Value::Number(number) => Some(number.as_f64().unwrap_or(0.0) != 0.0),
        _ => None,
    }
}

/// 读取字符串属性（大小写不敏感、支持别名）：字符串原样返回，其它标量返回 JSON 原文。
pub fn try_read_string(token: &Value, names: &[&str]) -> Option<String> {
    if !token.is_object() {
        return None;
    }

    for name in names {
        if let Some(child) = try_get_property(token, name) {
            if child.is_null() {
                continue;
            }
            return Some(read_raw_text(child));
        }
    }

    None
}

/// 标量统一转字符串：`null` 为 `None`；非正数归一为 `"0.00"`；布尔转 `"true"`/`"false"`。
pub fn read_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => {
            if number.as_f64().unwrap_or(0.0) <= 0.0 {
                Some("0.00".to_string())
            } else {
                Some(number.to_string())
            }
        }
        Value::Bool(value) => Some(value.to_string()),
        other => Some(read_raw_text(other)),
    }
}

/// 大小写不敏感的对象属性查找；非对象返回 `None`。
pub fn try_get_property<'a>(token: &'a Value, name: &str) -> Option<&'a Value> {
    token
        .as_object()?
        .iter()
        .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value))
}

fn looks_like_json(value: &str) -> bool {
    value.starts_with('{') || value.starts_with('[')
}

fn find_embedded_json_candidate(value: &str) -> Option<String> {
    if let Some(index) = value.find('{') {
        return Some(value[index..].trim().to_string());
    }
    value
        .find('[')
        .map(|index| value[index..].trim().to_string())
}

fn read_raw_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn enumerate_tokens_parses_json_lines_and_embedded_json() {
        let payload = "debug line\n{\"first\":1}{\"second\":2}\ninfo: {\"third\":3}";

        let tokens = enumerate_tokens(Some(payload));

        assert_eq!(tokens.len(), 3);
        assert_eq!(
            try_read_string(&tokens[0], &["first"]).as_deref(),
            Some("1")
        );
        assert_eq!(
            try_read_string(&tokens[1], &["second"]).as_deref(),
            Some("2")
        );
        assert_eq!(
            try_read_string(&tokens[2], &["third"]).as_deref(),
            Some("3")
        );
    }

    #[test]
    fn enumerate_tokens_parses_complete_array_as_single_token() {
        let tokens = enumerate_tokens(Some("[{\"success\":true}]"));

        assert_eq!(tokens.len(), 1);
        assert!(tokens[0].is_array());
        assert_eq!(try_read_boolean(&tokens[0], "success"), None);
    }

    #[test]
    fn enumerate_tokens_empty_payload_returns_no_tokens() {
        assert!(enumerate_tokens(None).is_empty());
        assert!(enumerate_tokens(Some("")).is_empty());
        assert!(enumerate_tokens(Some("   ")).is_empty());
    }

    #[test]
    fn enumerate_tokens_skips_lines_without_json() {
        assert!(enumerate_tokens(Some("plain\nwords only")).is_empty());
    }

    #[test]
    fn try_read_boolean_reads_boolean_string_and_numeric_values() {
        let token =
            try_parse_token("{\"one\":true,\"two\":\"false\",\"three\":2,\"four\":0}").unwrap();

        assert_eq!(try_read_boolean(&token, "ONE"), Some(true));
        assert_eq!(try_read_boolean(&token, "two"), Some(false));
        assert_eq!(try_read_boolean(&token, "three"), Some(true));
        assert_eq!(try_read_boolean(&token, "four"), Some(false));
    }

    #[test]
    fn try_read_boolean_rejects_missing_null_and_non_boolean_values() {
        let token = try_parse_token("{\"empty\":null,\"text\":\"unknown\"}").unwrap();

        assert_eq!(try_read_boolean(&token, "empty"), None);
        assert_eq!(try_read_boolean(&token, "text"), None);
        assert_eq!(try_read_boolean(&token, "missing"), None);
    }

    #[test]
    fn try_read_string_uses_aliases_and_returns_scalar_values() {
        let token =
            try_parse_token("{\"EMAIL\":\"user@example.com\",\"id\":42,\"enabled\":true}").unwrap();

        assert_eq!(
            try_read_string(&token, &["email", "eamil"]).as_deref(),
            Some("user@example.com")
        );
        assert_eq!(try_read_string(&token, &["id"]).as_deref(), Some("42"));
        assert_eq!(
            try_read_string(&token, &["enabled"]).as_deref(),
            Some("true")
        );
    }

    #[test]
    fn read_scalar_as_string_normalizes_numeric_values() {
        let cases = [
            (json!({"price": 0}), "0.00"),
            (json!({"price": -1}), "0.00"),
            (json!({"price": 12.5}), "12.5"),
        ];

        for (json, expected) in cases {
            let token = try_parse_token(&json.to_string()).unwrap();
            let price = token.get("price").unwrap();
            assert_eq!(read_scalar_as_string(price), Some(expected.to_string()));
        }
    }

    #[test]
    fn try_get_property_ignores_case_and_rejects_non_objects() {
        let token = try_parse_token("{\"Name\":\"value\"}").unwrap();

        assert_eq!(
            try_get_property(&token, "name").and_then(Value::as_str),
            Some("value")
        );
        assert_eq!(try_get_property(&Value::Null, "name"), None);
    }
}
