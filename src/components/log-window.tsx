import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Copy, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useLogs } from "@/stores/logs";
import { LOG_LEVEL_CLASS, logLevelTag } from "@/lib/status";
import { useRenderMessage } from "@/lib/messages";

// 日志独立窗口内容（对应原 LogViewerWindow：深色等宽、按等级着色、
// 新日志自动滚动到底部、复制/清空/关闭）。main.tsx 按窗口标签分流到此。
export function LogWindow() {
  const { t } = useTranslation();
  const renderMessage = useRenderMessage();
  const entries = useLogs((s) => s.entries);
  const setOpen = useLogs((s) => s.setOpen);
  const clear = useLogs((s) => s.clear);
  const bottomRef = useRef<HTMLDivElement>(null);
  const [hint, setHint] = useState("");

  // 窗口标题随语言变化（index.html 的 <title> 会覆盖 builder 的 title）
  useEffect(() => {
    document.title = t("LogViewerWindow/TitleBar.Title");
  }, [t]);

  useEffect(() => {
    void useLogs.getState().init();
  }, []);

  // CDP 调试用：暴露 store
  useEffect(() => {
    (window as unknown as Record<string, unknown>).__logs = useLogs;
  }, []);

  // 新日志到达时滚动到底部
  useEffect(() => {
    bottomRef.current?.scrollIntoView();
  }, [entries.length]);

  function flash(message: string) {
    setHint(message);
    setTimeout(() => setHint(""), 1500);
  }

  async function handleCopy() {
    if (entries.length === 0) {
      flash(t("Common/LogDialog/CopyEmptyLog"));
      return;
    }
    const text = entries
      .map((e) => `[${e.timestamp}] [${logLevelTag(e.level)}] ${renderMessage(e.message)}`)
      .join("\n");
    await navigator.clipboard.writeText(text);
    flash(t("Common/LogDialog/CopiedToClipboard"));
  }

  return (
    <div className="flex h-svh flex-col bg-background">
      <div className="flex-1 overflow-y-auto bg-zinc-950 p-3 font-mono text-xs leading-5">
        {entries.map((entry, index) => (
          <div key={index} className={LOG_LEVEL_CLASS[entry.level] ?? "text-zinc-300"}>
            [{entry.timestamp}] [{logLevelTag(entry.level)}] {renderMessage(entry.message)}
          </div>
        ))}
        <div ref={bottomRef} />
      </div>
      <div className="flex items-center gap-2 border-t px-4 py-3">
        {hint && (
          <span className="flex items-center gap-1 text-xs text-green-600">
            <Check className="size-3" />
            {hint}
          </span>
        )}
        <Button variant="outline" size="sm" onClick={() => void handleCopy()}>
          <Copy className="size-4" />
          {t("LogViewerWindow/CopyButton.Content")}
        </Button>
        <Button variant="outline" size="sm" onClick={clear}>
          <Trash2 className="size-4" />
          {t("LogViewerWindow/ClearButton.Content")}
        </Button>
        <div className="flex-1" />
        <Button variant="ghost" size="sm" onClick={() => setOpen(false)}>
          <X className="size-4" />
          {t("LogViewerWindow/CloseButton.Content")}
        </Button>
      </div>
    </div>
  );
}
