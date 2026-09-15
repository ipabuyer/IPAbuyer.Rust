import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogOut, Minus, Search, Square, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { useSession } from "@/stores/session";
import { useSearch } from "@/stores/search";
import { cn } from "@/lib/utils";
import type { PageKey } from "@/components/app-sidebar";

function WindowButton({
  onClick,
  danger,
  children,
}: {
  onClick: () => void;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size="icon"
      className={cn("size-8 rounded-none", danger && "hover:bg-destructive hover:text-white")}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

export function TitleBar({ active }: { active: PageKey }) {
  const { t } = useTranslation();
  const loggedIn = useSession((s) => s.loggedIn);
  const { query, setQuery, search } = useSearch();
  const appWindow = getCurrentWindow();

  return (
    <div
      data-tauri-drag-region
      className="flex h-10 shrink-0 items-center gap-1 border-b bg-sidebar pr-0 pl-2"
    >
      <SidebarTrigger />
      <span className="ml-1 text-sm font-medium">{t("MainWindow/TitleBar.Title")}</span>

      {/* 居中搜索框：仅主页显示（对应 WinUI3 标题栏 AutoSuggestBox） */}
      <div className="flex flex-1 justify-center">
        {active === "main" && (
          <div className="relative w-[320px] max-w-[40%]">
            <Search className="absolute top-1/2 left-2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              className="h-7 pl-8 text-sm"
              placeholder={t("MainWindow/SearchBox.PlaceholderText")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void search();
              }}
            />
          </div>
        )}
      </div>

      {/* 登录态头像：绿色已登录 / 红色未登录（M1 占位 null 为灰色） */}
      <Avatar
        className={cn(
          "mr-2 size-6 border",
          loggedIn === true && "border-2 border-green-600",
          loggedIn === false && "border-2 border-red-500",
        )}
      >
        <AvatarFallback className="text-[10px]">
          {loggedIn === false && <LogOut className="size-3" />}
        </AvatarFallback>
      </Avatar>

      <div className="flex h-10 items-center">
        <WindowButton onClick={() => void appWindow.minimize()}>
          <Minus className="size-4" />
        </WindowButton>
        <WindowButton onClick={() => void appWindow.toggleMaximize()}>
          <Square className="size-3" />
        </WindowButton>
        <WindowButton danger onClick={() => void appWindow.close()}>
          <X className="size-4" />
        </WindowButton>
      </div>
    </div>
  );
}
