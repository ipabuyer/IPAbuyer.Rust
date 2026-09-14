import { useEffect, useState } from "react";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { Toaster } from "@/components/ui/sonner";
import { AppSidebar, type PageKey } from "@/components/app-sidebar";
import { TitleBar } from "@/components/title-bar";
import { HomePage } from "@/pages/home";
import { AccountPage } from "@/pages/account";
import { IpatoolPage } from "@/pages/ipatool";
import { SettingsPage } from "@/pages/settings";
import { api } from "@/lib/api";
import { useSession } from "@/stores/session";

const PAGES = {
  main: HomePage,
  account: AccountPage,
  ipatool: IpatoolPage,
  settings: SettingsPage,
} as const;

export default function App() {
  const [page, setPage] = useState<PageKey>("main");
  const Page = PAGES[page];

  // 启动时静默恢复会话（对应原 C# WarmupAuthInfoAsync）
  // 显示语言由 Rust initialization_script 在首帧前注入，无需在此处理
  useEffect(() => {
    void api
      .authInfo()
      .then((info) => {
        if (info.status === "LoggedIn" && info.email) {
          useSession.getState().setSession(true, info.email, false);
        } else if (info.status === "NotLoggedIn") {
          useSession.getState().reset();
        }
      })
      .catch(() => {});
  }, []);

  return (
    <SidebarProvider>
      <AppSidebar active={page} onNavigate={setPage} />
      <SidebarInset className="h-svh flex-col overflow-hidden">
        <TitleBar active={page} />
        <main className="flex-1 overflow-y-auto">
          <Page />
        </main>
      </SidebarInset>
      <Toaster position="bottom-center" />
    </SidebarProvider>
  );
}
