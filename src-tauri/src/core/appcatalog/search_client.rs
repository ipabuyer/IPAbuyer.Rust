//! iTunes Search HTTP 客户端。
//!
//! 移植自主仓库 `AppleAppStoreSearchClient`：阻塞式请求（`ureq` + rustls），
//! 总超时 2 分钟；非 2xx 时错误体优先、键名兜底；请求中途不支持取消
//! （搜索耗时短，宿主侧以超时兜底，见 DEVELOPMENT.md 5.3）。

use std::time::Duration;

use crate::core::ipatool::response_parser::NormalizedText;
use crate::core::ipatool::result::{ErrorMessage, IpatoolResult};

const SEARCH_ENDPOINT: &str = "https://itunes.apple.com/search";
const USER_AGENT: &str = "IPAbuyer/1.0";
/// 搜索请求总超时（对齐 C# HttpClient 2 分钟超时）。
const SEARCH_TIMEOUT: Duration = Duration::from_secs(120);

/// 搜索 App Store；结果体在 `output`，HTTP/网络错误在 `error`。
pub fn search(name: &str, limit: i64, country_code: &str) -> IpatoolResult {
    let query = name.trim();
    if query.is_empty() {
        return IpatoolResult::from_error_message(
            ErrorMessage {
                key: "Ipatool/Error/AppNameRequired",
                args: Vec::new(),
            },
            -1,
            false,
        );
    }

    let country = if country_code.trim().is_empty() {
        "cn".to_string()
    } else {
        country_code.trim().to_lowercase()
    };
    let limit = limit.max(1);

    let agent = ureq::AgentBuilder::new()
        .timeout(SEARCH_TIMEOUT)
        .user_agent(USER_AGENT)
        .build();

    let request = agent
        .get(SEARCH_ENDPOINT)
        .query("term", query)
        .query("entity", "software")
        .query("limit", &limit.to_string())
        .query("country", &country);

    match request.call() {
        Ok(response) => {
            let content = response.into_string().unwrap_or_default();
            IpatoolResult::from_streams(
                NormalizedText::Raw(content),
                NormalizedText::Raw(String::new()),
                0,
            )
        }
        Err(ureq::Error::Status(code, response)) => {
            let reason = response.status_text().to_string();
            let content = response.into_string().unwrap_or_default();
            let error = if content.trim().is_empty() {
                NormalizedText::Keyed {
                    key: "Ipatool/Error/HttpRequestFailed",
                    args: vec![code.to_string(), reason],
                }
            } else {
                NormalizedText::Raw(content)
            };
            IpatoolResult::from_streams(NormalizedText::Raw(String::new()), error, i32::from(code))
        }
        Err(ureq::Error::Transport(transport)) => {
            // ureq 的超时以 Transport 错误上报（无独立 Timeout 类别），统一透传原始描述。
            IpatoolResult::from_streams(
                NormalizedText::Raw(String::new()),
                NormalizedText::Raw(transport.to_string()),
                -1,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_requires_non_empty_name() {
        let result = search("   ", 200, "cn");

        assert!(!result.timed_out);
        assert_eq!(
            result.error_message.expect("app name required").key,
            "Ipatool/Error/AppNameRequired"
        );
    }
}
