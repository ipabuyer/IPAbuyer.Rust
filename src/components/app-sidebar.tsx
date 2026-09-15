import { openUrl } from "@tauri-apps/plugin-opener";
import { CircleHelp, House, Package, Settings, UserRound } from "lucide-react";
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

const FAQ_URL = "https://ipa.blazesnow.com/faq.html";

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
  async function openFaq() {
    try {
      await openUrl(FAQ_URL);
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
              <SidebarMenuItem>
                <SidebarMenuButton
                  tooltip={t("MainWindow/Nav/Faq.Content")}
                  onClick={() => void openFaq()}
                >
                  <CircleHelp />
                  <span>{t("MainWindow/Nav/Faq.Content")}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
}
