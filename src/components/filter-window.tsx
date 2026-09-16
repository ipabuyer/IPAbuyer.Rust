import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/api";
import type { PlatformFilter } from "@/lib/status";
import { useFilter } from "@/stores/filter";
import { cn } from "@/lib/utils";

const PLATFORM_KEY: Record<PlatformFilter, string> = {
  all: "MainPage/Filter/AllPlatforms",
  ios: "MainPage/Platform/Ios",
  ipad: "MainPage/Platform/Ipad",
  macos: "MainPage/Platform/Macos",
};

// 独立筛选窗口（label filter，仿 LogViewerWindow）：平台 + 开发者，
// 条目即时生效并经后端推送回主窗口；窗口由 Rust 端按需创建/隐藏。
export function FilterWindow() {
  const { t } = useTranslation();
  const { platform, developer, developers, setPlatform, setDeveloper } = useFilter();

  // 窗口标题随语言变化（index.html 的 <title> 会覆盖 builder 的 title）
  useEffect(() => {
    document.title = t("MainPage/Action/FilterButton.Content");
  }, [t]);

  useEffect(() => {
    void useFilter.getState().init();
  }, []);

  return (
    <div className="flex h-svh flex-col bg-background">
      <div className="flex-1 space-y-4 overflow-y-auto p-4">
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">{t("MainPage/Filter/Platform")}</Label>
          <div className="flex h-8 w-fit overflow-hidden rounded-md border">
            {(["all", "ios", "ipad", "macos"] as PlatformFilter[]).map((key) => (
              <button
                key={key}
                className={cn(
                  "h-full px-3 text-xs",
                  platform === key ? "bg-primary text-primary-foreground" : "hover:bg-accent",
                )}
                onClick={() => setPlatform(key)}
              >
                {t(PLATFORM_KEY[key])}
              </button>
            ))}
          </div>
        </div>
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">{t("MainPage/Filter/Developer")}</Label>
          <Select value={developer} onValueChange={setDeveloper}>
            <SelectTrigger size="sm" className="w-full text-xs">
              <SelectValue placeholder={t("MainPage/DeveloperSelectorAllItem.Content")} />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t("MainPage/DeveloperSelectorAllItem.Content")}</SelectItem>
              {developers.map((name) => (
                <SelectItem key={name} value={name}>
                  {name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="flex items-center border-t px-4 py-3">
        <div className="flex-1" />
        <Button variant="outline" size="sm" onClick={() => void api.filterHide()}>
          {t("LogViewerWindow/CloseButton.Content")}
        </Button>
      </div>
    </div>
  );
}
