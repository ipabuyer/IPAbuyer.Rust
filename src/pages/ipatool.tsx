import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Check,
  ExternalLink,
  Trash2,
  Info,
  Loader2,
  Package,
  TriangleAlert,
  Upload,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { SettingsCard } from "@/components/settings-card";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";

const IPATOOL_REPO = "https://github.com/majd/ipatool";

interface IpatoolInfo {
  flavor: "main" | "custom";
  customPath: string | null;
  builtinVersion: string;
  activePath: string;
  builtinAvailable: boolean;
}

export function IpatoolPage() {
  const { t } = useTranslation();
  const [info, setInfo] = useState<IpatoolInfo | null>(null);
  const [detailedLog, setDetailedLog] = useState(false);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<null | "export" | "clear">(null);

  useEffect(() => {
    void reload();
    void api.getSettings().then((c) => setDetailedLog(c.detailedIpatoolLog));
  }, []);

  async function reload() {
    setInfo(await api.ipatoolInfo().catch(() => null));
  }

  async function persist(action: () => Promise<unknown>, reloadAfter = true) {
    setBusy(true);
    try {
      await action();
      if (reloadAfter) await reload();
    } catch (error) {
      toast.error(t("IpatoolPage/Custom/SaveFailMessage"), { description: String(error) });
    } finally {
      setBusy(false);
    }
  }

  async function handlePickCustom() {
    const selection = await open({
      multiple: false,
      filters: [{ name: "ipatool", extensions: ["exe"] }],
    });
    if (typeof selection !== "string") return;
    await persist(() => api.ipatoolSetCustomPath(selection));
  }

  async function handleExport() {
    setBusy(true);
    try {
      const target = await api.ipatoolExport();
      toast.success(t("IpatoolPage/Export/SuccessMessage"), { description: target });
    } catch (error) {
      toast.error(t("IpatoolPage/Export/FailMessage"), { description: String(error) });
    } finally {
      setBusy(false);
      setConfirm(null);
    }
  }

  async function handleClearData() {
    setBusy(true);
    try {
      await api.ipatoolClearData();
      toast.success(t("IpatoolPage/Data/ClearSuccessMessage"));
    } catch (error) {
      toast.error(t("IpatoolPage/Data/ClearFailMessage"), { description: String(error) });
    } finally {
      setBusy(false);
      setConfirm(null);
    }
  }

  async function handleToggleDetailedLog(checked: boolean) {
    setDetailedLog(checked);
    await persist(() => api.setDetailedLog(checked), false);
  }

  async function openRepo() {
    try {
      await openUrl(IPATOOL_REPO);
    } catch (error) {
      toast.error(String(error));
    }
  }

  if (!info) {
    return (
      <div className="flex justify-center p-10">
        <Loader2 className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const usingBuiltin = info.flavor === "main";
  const useButton = t("IpatoolPage/Custom/UseButton.Content");

  return (
    <div className="mx-auto w-full max-w-2xl space-y-4 p-6">
      <div>
        <h1 className="text-lg font-semibold">{t("MainWindow/Nav/Ipatool.Content")}</h1>
        <p className="text-sm text-muted-foreground">{t("IpatoolPage/DescriptionTextBlock.Text")}</p>
      </div>

      <div className="space-y-2">
        {/* 内置版本卡片 */}
        <SettingsCard
          icon={Package}
          header={t("IpatoolPage/Card/Release.Header")}
          description={t("IpatoolPage/Release/DisplayName")}
        >
          <span className="text-sm text-muted-foreground">
            release@{info.builtinVersion}
          </span>
          {usingBuiltin && info.builtinAvailable && (
            <span className="flex items-center gap-1 rounded-full bg-green-100 px-2 py-0.5 text-xs text-green-700 dark:bg-green-900/40 dark:text-green-400">
              <Check className="size-3" />
              {t("IpatoolPage/Badge/Current")}
            </span>
          )}
          <Button
            variant="outline"
            size="sm"
            disabled={busy || !info.builtinAvailable}
            onClick={() => setConfirm("export")}
          >
            {t("IpatoolPage/Release/Menu/Export.Text")}
          </Button>
        </SettingsCard>

        {/* 自定义 ipatool 卡片 */}
        <SettingsCard
          icon={Upload}
          header={t("IpatoolPage/Card/Custom.Header")}
          description={info.customPath ?? t("IpatoolPage/Custom/EmptyPath")}
        >
          {!usingBuiltin && (
            <span className="flex items-center gap-1 rounded-full bg-green-100 px-2 py-0.5 text-xs text-green-700 dark:bg-green-900/40 dark:text-green-400">
              <Check className="size-3" />
              {t("IpatoolPage/Badge/Current")}
            </span>
          )}
          <Button variant="outline" size="sm" disabled={busy} onClick={() => void handlePickCustom()}>
            {info.customPath ? t("IpatoolPage/Button/Replace") : t("IpatoolPage/Button/Pick")}
          </Button>
          {info.customPath && (
            <Button
              variant="outline"
              size="sm"
              disabled={busy || !usingBuiltin}
              onClick={() => void persist(() => api.ipatoolSetFlavor("custom"))}
            >
              {useButton}
            </Button>
          )}
          {info.customPath && (
            <Button
              variant="ghost"
              size="icon"
              className="size-8 text-red-500"
              disabled={busy}
              onClick={() => void persist(() => api.ipatoolDeleteCustom())}
            >
              <Trash2 className="size-4" />
            </Button>
          )}
        </SettingsCard>

        {/* 版本要求卡片 */}
        <SettingsCard
          icon={Info}
          header={t("IpatoolPage/Card/VersionRequirement.Header")}
          description={t("IpatoolPage/Card/VersionRequirement.Description")}
        >
          <span
            className={cn(
              "text-sm font-medium",
              info.builtinAvailable ? "text-green-600" : "text-red-500",
            )}
          >
            {t("IpatoolPage/VersionRequirement/MinimumVersionTextBlock.Text")}
          </span>
        </SettingsCard>

        {/* 详细日志开关 */}
        <SettingsCard
          header={t("IpatoolPage/Card/DetailedIpatoolLog.Header")}
          description={t("IpatoolPage/Card/DetailedIpatoolLog.Description")}
        >
          <Switch checked={detailedLog} onCheckedChange={(c) => void handleToggleDetailedLog(c)} />
        </SettingsCard>

        {/* 清空 ipatool 数据 */}
        <SettingsCard
          icon={Trash2}
          header={t("IpatoolPage/Card/ClearIpatoolData.Header")}
          description={t("IpatoolPage/Card/ClearIpatoolData.Description")}
        >
          <Button variant="outline" size="sm" disabled={busy} onClick={() => setConfirm("clear")}>
            {t("IpatoolPage/ClearIpatoolDataButton.Content")}
          </Button>
        </SettingsCard>

        {/* 仓库链接 */}
        <SettingsCard
          icon={ExternalLink}
          header={t("IpatoolPage/Card/Repository.Header")}
          description={t("IpatoolPage/Card/Repository.Description")}
        >
          <Button variant="outline" size="sm" onClick={() => void openRepo()}>
            {t("IpatoolPage/Repository/OpenButton.Content")}
          </Button>
        </SettingsCard>
      </div>

      {/* 导出确认 */}
      <Dialog open={confirm === "export"} onOpenChange={(o) => !o && setConfirm(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("IpatoolPage/Export/ConfirmTitle")}</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">{t("IpatoolPage/Export/ConfirmMessage")}</p>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirm(null)}>
              {t("Settings/CountryCode/CancelButton")}
            </Button>
            <Button onClick={() => void handleExport()}>
              {t("IpatoolPage/Export/ConfirmPrimary")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 清空数据确认 */}
      <Dialog open={confirm === "clear"} onOpenChange={(o) => !o && setConfirm(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <TriangleAlert className="size-4 text-amber-500" />
              {t("IpatoolPage/Card/ClearIpatoolData.Header")}
            </DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">{t("IpatoolPage/Data/ClearConfirmMessage")}</p>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirm(null)}>
              {t("Settings/CountryCode/CancelButton")}
            </Button>
            <Button variant="destructive" onClick={() => void handleClearData()}>
              {t("IpatoolPage/ClearIpatoolDataButton.Content")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
