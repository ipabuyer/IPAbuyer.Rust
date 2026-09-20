//! iTunes Search HTTP 客户端。
//!
//! 移植自主仓库 `AppleAppStoreSearchClient`：阻塞式请求（`ureq` + rustls），
//! 总超时 2 分钟；非 2xx 时错误体优先、键名兜底；请求中途不支持取消
//! （搜索耗时短，宿主侧以超时兜底，见 references/features.md「搜索功能」）。

use std::time::Duration;

use crate::core::ipatool::response_parser::NormalizedText;
use crate::core::ipatool::result::{ErrorMessage, IpatoolResult};

const SEARCH_ENDPOINT: &str = "https://itunes.apple.com/search";
const USER_AGENT: &str = "IPAbuyer/1.0";
/// 搜索请求总超时（对齐 C# HttpClient 2 分钟超时）。
const SEARCH_TIMEOUT: Duration = Duration::from_secs(120);

/// iTunes Search entity：各 App Store 是独立实体，同一请求只能选其一
/// （多平台结果需分别请求后合并；tvOS/visionOS 无公开实体）。
pub const ENTITY_SOFTWARE: &str = "software";
pub const ENTITY_IPAD_SOFTWARE: &str = "iPadSoftware";
pub const ENTITY_MAC_SOFTWARE: &str = "macSoftware";

/// 搜索 App Store；结果体在 `output`，HTTP/网络错误在 `error`。
pub fn search(name: &str, limit: i64, country_code: &str, entity: &str) -> IpatoolResult {
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

    // ureq 3：非 2xx 默认转 Err 且不带响应体，关闭后统一在 Ok 分支内
    // 按 status 处理，保持错误体优先的解析逻辑。
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .user_agent(USER_AGENT)
            .timeout_global(Some(SEARCH_TIMEOUT))
            .http_status_as_error(false)
            .build(),
    );

    match agent
        .get(SEARCH_ENDPOINT)
        .query("term", query)
        .query("entity", entity)
        .query("limit", &limit.to_string())
        .query("country", &country)
        .call()
    {
        Ok(response) => {
            let status = response.status();
            let code = status.as_u16();
            let reason = status.canonical_reason().unwrap_or_default();
            let content = response.into_body().read_to_string().unwrap_or_default();
            if (200..300).contains(&code) {
                IpatoolResult::from_streams(
                    NormalizedText::Raw(content),
                    NormalizedText::Raw(String::new()),
                    0,
                )
            } else {
                let error = if content.trim().is_empty() {
                    NormalizedText::Keyed {
                        key: "Ipatool/Error/HttpRequestFailed",
                        args: vec![code.to_string(), reason.to_string()],
                    }
                } else {
                    NormalizedText::Raw(content)
                };
                IpatoolResult::from_streams(NormalizedText::Raw(String::new()), error, i32::from(code))
            }
        }
        Err(err) => {
            // 网络层错误（连接/IO/超时/协议），统一透传原始描述。
            IpatoolResult::from_streams(
                NormalizedText::Raw(String::new()),
                NormalizedText::Raw(err.to_string()),
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
        let result = search("   ", 200, "cn", ENTITY_SOFTWARE);

        assert!(!result.timed_out);
        assert_eq!(
            result.error_message.expect("app name required").key,
            "Ipatool/Error/AppNameRequired"
        );
    }
}
