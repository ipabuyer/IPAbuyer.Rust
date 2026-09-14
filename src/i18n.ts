import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zhHans from "./locales/zh-Hans.json";
import enUS from "./locales/en-US.json";

// key 原样查找：resw 迁移的 key 含 "/" 与 "."（属性后缀），禁用 i18next 分隔符
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

// C# string.Format 风格占位符：core 消息与 resw 文案均使用 {0}/{1}
export function fmt(template: string, ...args: (string | number)[]): string {
  return template.replace(/\{(\d+)\}/g, (m, i) => String(args[Number(i)] ?? m));
}

export default i18n;
