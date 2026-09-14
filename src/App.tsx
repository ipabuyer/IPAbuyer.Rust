import { useState } from "react";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar, type PageKey } from "@/components/app-sidebar";
import { TitleBar } from "@/components/title-bar";
import { HomePage } from "@/pages/home";
import { AccountPage } from "@/pages/account";
import { IpatoolPage } from "@/pages/ipatool";
import { SettingsPage } from "@/pages/settings";

const PAGES = {
  main: HomePage,
  account: AccountPage,
  ipatool: IpatoolPage,
  settings: SettingsPage,
} as const;

export default function App() {
  const [page, setPage] = useState<PageKey>("main");
  const Page = PAGES[page];

  return (
    <SidebarProvider>
      <AppSidebar active={page} onNavigate={setPage} />
      <SidebarInset className="h-svh flex-col overflow-hidden">
        <TitleBar active={page} />
        <main className="flex-1 overflow-y-auto">
          <Page />
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
