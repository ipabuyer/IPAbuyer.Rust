import { House, Package, Settings, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
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
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
}
