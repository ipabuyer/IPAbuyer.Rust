import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Copy, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { useLogs } from "@/stores/logs";
import { LOG_LEVEL_CLASS, logLevelTag } from "@/lib/status";
import { useRenderMessage } from "@/lib/messages";

// 日志侧滑面板（对应原 LogViewerWindow：深色等宽、按等级着色、复制/清空/关闭）
export function LogSheet({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { t } = useTranslation();
  const renderMessage = useRenderMessage();
  const entries = useLogs((s) => s.entries);
  const clear = useLogs((s) => s.clear);
  const [hint, setHint] = useState("");

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
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="flex w-[560px] max-w-[60vw] flex-col gap-0 p-0 sm:max-w-[60vw]">
        <SheetHeader className="border-b px-4 py-3">
          <SheetTitle className="text-sm">{t("LogViewerWindow/TitleBar.Title")}</SheetTitle>
        </SheetHeader>
        <div className="flex-1 overflow-y-auto bg-zinc-950 p-3 font-mono text-xs leading-5">
          {entries.map((entry, index) => (
            <div key={index} className={LOG_LEVEL_CLASS[entry.level] ?? "text-zinc-300"}>
              [{entry.timestamp}] [{logLevelTag(entry.level)}] {renderMessage(entry.message)}
            </div>
          ))}
        </div>
        <SheetFooter className="flex-row items-center gap-2 border-t px-4 py-3">
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
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            <X className="size-4" />
            {t("LogViewerWindow/CloseButton.Content")}
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
