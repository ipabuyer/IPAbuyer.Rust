import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zhHans from "./locales/zh-Hans.json";
import enUS from "./locales/en-US.json";

// key 原样查找：resw 迁移的 key 含 "/" 与 "."（属性后缀），禁用 i18next 分隔符
// 占位符使用 i18next 原生插值（{{0}} 位置参数，与 core 消息的位置参数数组契约一致）
void i18n.use(initReactI18next).init({
  resources: {
    "zh-Hans": { translation: zhHans },
    "en-US": { translation: enUS },
  },
  lng: navigator.language.toLowerCase().startsWith("zh") ? "zh-Hans" : "en-US",
  fallbackLng: "zh-Hans",
  keySeparator: false,
  nsSeparator: false,
  interpolation: { escapeValue: false },
});

export default i18n;
