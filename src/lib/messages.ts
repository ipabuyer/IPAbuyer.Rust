import { useTranslation } from "react-i18next";
import type { JsMessage } from "./types";

/**
 * 渲染后端消息：key → i18next（{{0}} 位置插值），raw → 原文。
 * core 消息键名约定与 resw 一致，无对应键时 i18next 回退显示键名。
 */
export function useRenderMessage() {
  const { t } = useTranslation();
  return (message: JsMessage | null | undefined): string => {
    if (!message) return "";
    if (message.type === "raw") return message.text;
    const args = Object.fromEntries(message.args.map((value, i) => [String(i), value]));
    return t(message.key, args);
  };
}
