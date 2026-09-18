import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zhHans from "./locales/zh-Hans.json";
import enUS from "./locales/en-US.json";

// key 原样查找：resw 迁移的 key 含 "/" 与 "."（属性后缀），禁用 i18next 分隔符
// 占位符使用 i18next 原生插值（{{0}} 位置参数，与 core 消息的位置参数数组契约一致）
// 初始语言：Rust initialization_script 在页面脚本前注入持久化偏好；未注入时按系统语言推断
const bootLang = (window as { __IPABUYER_LANG__?: string }).__IPABUYER_LANG__;

/// 解析显示语言偏好："auto"（或空）按系统语言推断，其余原值。
export function resolveLanguage(value?: string): string {
  const trimmed = value?.trim();
  if (trimmed && trimmed !== "auto") return trimmed;
  return navigator.language.toLowerCase().startsWith("zh") ? "zh-Hans" : "en-US";
}

void i18n.use(initReactI18next).init({
  resources: {
    "zh-Hans": { translation: zhHans },
    "en-US": { translation: enUS },
  },
  lng: resolveLanguage(bootLang),
  fallbackLng: "zh-Hans",
  keySeparator: false,
  nsSeparator: false,
  interpolation: { escapeValue: false },
});

export default i18n;
