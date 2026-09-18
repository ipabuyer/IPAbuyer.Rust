//! 搜索命令：iTunes Search + 已购状态合成（core 承担）。

use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::JsMessage;
use crate::core::appcatalog::search_parser::SearchResult;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultDto {
    pub bundle_id: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub developer: Option<String>,
    pub artwork_url: Option<String>,
    pub price: String,
    pub version: Option<String>,
    pub platform: String,
    pub purchased: String,
}

fn to_dto(result: SearchResult) -> SearchResultDto {
    SearchResultDto {
        bundle_id: result.bundle_id,
        id: result.id,
        name: result.name,
        developer: result.developer,
        artwork_url: result.artwork_url,
        price: result.price,
        version: result.version,
        platform: result.platform,
        purchased: result.purchased,
    }
}

/// 搜索 App Store；超时或空响应返回空列表。
/// 未登录时不合成已购状态（全部为搜索原始状态）。
/// 搜索完成后刷新筛选窗口的开发者选项（失效选择自动回退）。
/// 同步命令默认在主线程执行（三次 HTTP 请求会冻结 UI/光标），
/// 标记 `(async)` 使其运行在独立线程。
#[tauri::command(async)]
pub fn catalog_search(
    app: AppHandle,
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<SearchResultDto>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let (country_code, account) = {
        let config = state.config.lock().unwrap();
        let session = state.session.lock().unwrap();
        (
            config.country_code.clone(),
            if session.logged_in {
                session.account.clone()
            } else {
                None
            },
        )
    };

    // 已购查找键为「平台:bundleId」组合键（同一 bundleId 在两个商店是不同条目）。
    let lang = crate::i18n::Lang::from_state(&state);
    let purchased: HashMap<String, String> = match &account {
        Some(account) => state
            .db
            .lock()
            .unwrap()
            .as_ref()
            .ok_or_else(|| lang.message("error-db-not-initialized"))?
            .get_purchased_apps(account)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(app_id, status, platform)| {
                (crate::core::platform::purchase_key(&platform, &app_id), status)
            })
            .collect(),
        None => HashMap::new(),
    };

    let results = crate::core::appcatalog::catalog_service::search_catalog(
        &query,
        200,
        &country_code,
        &|code| crate::storefront::contains(code),
        &purchased,
    );

    // 开发者选项随搜索结果刷新（推送给筛选窗口与主窗口）。
    let developers = results
        .as_ref()
        .map(|list| {
            build_developer_options(
                list.iter()
                    .filter_map(|item| item.developer.clone())
                    .collect(),
            )
        })
        .unwrap_or_default();
    crate::commands::filter::refresh_developer_options(&app, &state, developers);

    Ok(results
        .unwrap_or_default()
        .into_iter()
        .map(to_dto)
        .collect())
}

/// 开发者筛选选项（去重、忽略空名，保持出现顺序）。
pub fn build_developer_options(names: Vec<String>) -> Vec<String> {
    let mut options: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let lowered = name.to_lowercase();
        if !options.iter().any(|o| o.to_lowercase() == lowered) {
            options.push(name);
        }
    }
    options
}

/// 供前端渲染的错误消息包装。
pub fn error_message(text: impl Into<String>) -> JsMessage {
    JsMessage::raw(text)
}
