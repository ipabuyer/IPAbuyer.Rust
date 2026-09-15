import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowUpRight, CircleHelp, House, Package, Settings, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";

export type PageKey = "main" | "account" | "ipatool" | "settings";

// 指向外部网页的侧边栏项（菜单按钮右端带 ArrowUpRight 标识区分）
const EXTERNAL_NAV = [
  { icon: CircleHelp, label: "MainWindow/Nav/Faq.Content", url: "https://ipa.blazesnow.com/faq.html" },
] as const;

const NAV = [
  { key: "main", icon: House, label: "MainWindow/Nav/Main.Content" },
  { key: "account", icon: UserRound, label: "MainWindow/Nav/Account.Content" },
  { key: "ipatool", icon: Package, label: "MainWindow/Nav/Ipatool.Content" },
  { key: "settings", icon: Settings, label: "MainWindow/Nav/Settings.Content" },
] as const;

export function AppSidebar({
  active,
  onNavigate,
}: {
  active: PageKey;
  onNavigate: (page: PageKey) => void;
}) {
  const { t } = useTranslation();
  async function openExternal(url: string) {
    try {
      await openUrl(url);
    } catch (error) {
      toast.error(String(error));
    }
  }
  return (
    <Sidebar collapsible="icon">
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {NAV.map(({ key, icon: Icon, label }) => (
                <SidebarMenuItem key={key}>
                  <SidebarMenuButton
                    isActive={active === key}
                    onClick={() => onNavigate(key)}
                    tooltip={t(label)}
                  >
                    <Icon />
                    <span>{t(label)}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
              {EXTERNAL_NAV.map(({ icon: Icon, label, url }) => (
                <SidebarMenuItem key={label}>
                  <SidebarMenuButton
                    tooltip={t(label)}
                    onClick={() => void openExternal(url)}
                  >
                    <Icon />
                    <span>{t(label)}</span>
                    <ArrowUpRight className="ml-auto size-3.5 shrink-0 text-muted-foreground group-data-[collapsible=icon]:hidden" />
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
}
