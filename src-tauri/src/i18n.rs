//! 后端 i18n（Fluent）：本地化 Rust 侧自身生成的用户可见文本——命令错误、
//! 子窗口标题。core 消息保持键名 + 位置参数由前端渲染（references/localization.md），
//! 不经本模块。
//!
//! 语言偏好来自 settings 的 display_language（auto 按系统 locale 解析，
//! 对齐前端 initialize 注入行为）；文案缺失时由 static_loader 回退 zh-Hans。
//! 资源文件位于 `src-tauri/locales/{zh-Hans,en-US}/main.ftl`，编译期内嵌。

use std::collections::HashMap;
use std::borrow::Cow;

use fluent_templates::{static_loader, Loader};
use unic_langid::{langid, LanguageIdentifier};

use crate::state::AppState;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "zh-Hans",
    };
}

pub const ZH_HANS: &str = "zh-Hans";
pub const EN_US: &str = "en-US";

/// 后端文案语言。取值只可能是两个受支持语言的常量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lang(LanguageIdentifier);

impl Lang {
    pub const ZH_HANS: Lang = Lang(langid!("zh-Hans"));
    pub const EN_US: Lang = Lang(langid!("en-US"));

    /// 按 settings 的 display_language 解析（auto/未知 → 系统语言）。
    pub fn from_config(display_language: &str) -> Self {
        match display_language.trim() {
            EN_US => Self::EN_US,
            ZH_HANS => Self::ZH_HANS,
            _ => Self::system(),
        }
    }

    /// 按系统 locale 解析；无 locale 信息时回退中文（应用主市场，
    /// 对齐 static_loader 的 fallback_language）。
    pub fn system() -> Self {
        let zh = sys_locale::get_locale()
            .map(|locale| locale.to_lowercase().starts_with("zh"))
            .unwrap_or(true);
        if zh { Self::ZH_HANS } else { Self::EN_US }
    }

    /// 从应用状态解析（读 settings 的 display_language）。
    pub fn from_state(state: &AppState) -> Self {
        Self::from_config(&state.config.lock().unwrap().display_language)
    }

    pub fn as_str(&self) -> &'static str {
        if self.0 == langid!("zh-Hans") { ZH_HANS } else { EN_US }
    }

    /// 查无占位符的消息。
    pub fn message(&self, key: &str) -> String {
        LOCALES.lookup(&self.0, key)
    }

    /// 查带命名参数的消息：`{$name}` 占位符对应 `args` 中的同名条目。
    pub fn message_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        let map: HashMap<Cow<'static, str>, fluent_templates::fluent_bundle::FluentValue> = args
            .iter()
            .map(|(name, value)| {
                (
                    Cow::Owned(name.to_string()),
                    fluent_templates::fluent_bundle::FluentValue::from(value.to_string()),
                )
            })
            .collect();
        LOCALES.lookup_with_args(&self.0, key, &map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_config_maps_supported_and_falls_back_to_system() {
        assert_eq!(Lang::from_config("zh-Hans"), Lang::ZH_HANS);
        assert_eq!(Lang::from_config(" en-US "), Lang::EN_US);
        // auto/未知值走系统解析，两种结果都合法
        assert!(matches!(
            Lang::from_config("auto"),
            Lang::ZH_HANS | Lang::EN_US
        ));
    }

    #[test]
    fn message_localizes_in_both_languages() {
        let zh = Lang::ZH_HANS.message("error-not-signed-in");
        let en = Lang::EN_US.message("error-not-signed-in");

        assert_eq!(zh, "未登录。");
        assert_eq!(en, "Not signed in.");
    }

    #[test]
    fn message_with_interpolates_named_args() {
        let zh = Lang::ZH_HANS.message_with(
            "error-invalid-ipatool-flavor",
            &[("flavor", "junk")],
        );

        assert!(zh.contains("junk"), "插值缺失: {zh}");
        assert!(zh.contains("无效的 ipatool 来源"), "zh 文案缺失: {zh}");
    }

    #[test]
    fn en_missing_key_falls_back_to_zh_hans() {
        // en-US 资源缺失的键回退 zh-Hans（static_loader fallback_language）
        let value = Lang::EN_US.message("error-key-only-in-zh");
        assert_eq!(value, "仅中文资源占位");
    }
}
