import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, FolderOpen, Globe, KeyRound, Languages, Loader2, RotateCcw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { SettingsCard } from "@/components/settings-card";
import { RefreshCw, Database } from "lucide-react";
import { useLogs } from "@/stores/logs";
import { useSession } from "@/stores/session";
import { api } from "@/lib/api";
import type { AppConfig, Storefront } from "@/lib/types";

const DEVELOPER_SITE = "https://ipa.blazesnow.com";
const PROJECT_REPO = "https://github.com/ipabuyer/ipabuyer";

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [version, setVersion] = useState("");
  const [countryOpen, setCountryOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [defaultDir, setDefaultDir] = useState("");
  const [lastSync, setLastSync] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [legacyExists, setLegacyExists] = useState(false);

  useEffect(() => {
    void api.getSettings().then(setConfig);
    void api.defaultDownloadDirectory().then(setDefaultDir);
    void getVersion().then(setVersion);
    void api.legacyDbExists().then(setLegacyExists);
    void refreshLastSync();
  }, []);

  function refreshLastSync() {
    void api
      .syncLastTime()
      .then(setLastSync)
      .catch(() => setLastSync(null));
  }

  async function persist(action: Promise<AppConfig>, successMessage?: string) {
    setSaving(true);
    try {
      const next = await action;
      setConfig(next);
      if (successMessage) toast.success(successMessage);
    } catch (error) {
      toast.error(String(error));
    } finally {
      setSaving(false);
    }
  }

  async function handleLanguageChange(language: string) {
    await persist(api.setDisplayLanguage(language));
    // 即时生效，无需重启（auto 时按系统语言解析）
    const resolved =
      language === "auto"
        ? navigator.language.toLowerCase().startsWith("zh")
          ? "zh-Hans"
          : "en-US"
        : language;
    await i18n.changeLanguage(resolved);
  }

  async function handlePickDownloadDirectory() {
    const selection = await open({ directory: true, multiple: false });
    if (typeof selection !== "string") return;
    await persist(
      api.setDownloadDirectory(selection),
      t("Settings/DownloadDirectory/UpdatedMessage", { 0: selection }),
    );
  }

  async function handleResetDownloadDirectory() {
    try {
      const next = await api.resetDownloadDirectory();
      setConfig(next);
      const fallback = await api.defaultDownloadDirectory();
      toast.success(t("Settings/DownloadDirectory/ResetSuccessMessage", { 0: fallback }));
    } catch (error) {
      toast.error(t("Settings/DownloadDirectory/ResetFailMessage", { 0: String(error) }));
    }
  }

  async function handleSync() {
    const session = useSession.getState();
    if (session.loggedIn !== true) {
      toast.warning(t("Settings/PurchaseSync/LoginRequiredTitle"), {
        description: t("Settings/PurchaseSync/LoginRequiredMessage"),
      });
      return;
    }
    if (session.isMock) {
      toast.info(t("Settings/PurchaseSync/MockTitle"), {
        description: t("Settings/PurchaseSync/MockMessage"),
      });
      return;
    }
    if (syncing) {
      useLogs.getState().setOpen(true);
      return;
    }
    setSyncing(true);
    useLogs.getState().setOpen(true);
    try {
      const result = await api.syncStart();
      switch (result.outcome) {
        case "Completed":
          toast.success(t("Settings/PurchaseSync/ProgressFormat", { 0: result.synced, 1: result.total }));
          break;
        case "Canceled":
          toast.info(t("Common/Cancel"));
          break;
        case "AlreadyRunning":
          toast.info(t("Settings/PurchaseSync/Running"));
          break;
        case "Mock":
          toast.info(t("Settings/PurchaseSync/MockMessage"));
          break;
        default:
          toast.error(t("Settings/PurchaseSync/FailedMessage"), {
            description: result.message ?? undefined,
          });
      }
    } catch (error) {
      toast.error(t("Settings/PurchaseSync/FailedMessage"), { description: String(error) });
    } finally {
      setSyncing(false);
      refreshLastSync();
    }
  }

  async function handleLegacyImport() {
    try {
      await api.legacyDbImport();
      setLegacyExists(false);
      toast.success(t("Settings/Database/Clear/SuccessMessage"));
    } catch (error) {
      toast.error(String(error));
    }
  }

  async function openLink(url: string) {
    try {
      await openUrl(url);
    } catch (error) {
      toast.error(String(error));
    }
  }

  if (!config) {
    return (
      <div className="flex justify-center p-10">
        <Loader2 className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const downloadDir = config.downloadDirectory ?? defaultDir;

  return (
    <div className="mx-auto w-full max-w-2xl space-y-4 p-6">
      <h1 className="text-lg font-semibold">{t("MainWindow/Nav/Settings.Content")}</h1>

      <div className="space-y-2">
        <SettingsCard
          icon={Languages}
          header={t("Settings/Card/DisplayLanguage.Header")}
          description={t("Settings/Card/DisplayLanguage.Description")}
        >
          <Select value={config.displayLanguage} onValueChange={(v) => void handleLanguageChange(v)}>
            <SelectTrigger className="w-36" disabled={saving}>
              <SelectValue>
                {config.displayLanguage === "zh-Hans"
                  ? "简体中文"
                  : config.displayLanguage === "en-US"
                    ? "English"
                    : t("Settings/Language/Auto.Content")}
              </SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="auto">{t("Settings/Language/Auto.Content")}</SelectItem>
              <SelectItem value="zh-Hans">简体中文</SelectItem>
              <SelectItem value="en-US">English</SelectItem>
            </SelectContent>
          </Select>
        </SettingsCard>

        <SettingsCard
          icon={Globe}
          header={t("Settings/Card/CountryCode.Header")}
          description={t("Settings/Card/CountryCode.Description")}
        >
          <span className="text-sm text-muted-foreground">
            {t("Settings/CountryCode/CurrentFormat", { 0: config.countryCode.toUpperCase() })}
          </span>
          <Button variant="outline" size="sm" onClick={() => setCountryOpen(true)}>
            {t("Settings/Button/EditCountryCode.Content")}
          </Button>
        </SettingsCard>

        <SettingsCard
          icon={FolderOpen}
          header={t("Settings/Card/DownloadDirectory.Header")}
          description={t("Settings/Card/DownloadDirectory.Description")}
        >
          <span className="max-w-64 truncate text-xs text-muted-foreground" title={downloadDir}>
            {downloadDir}
          </span>
          <Button variant="outline" size="sm" onClick={() => void handlePickDownloadDirectory()}>
            {t("Settings/Button/PickDownloadDirectory.Content")}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            title={t("Settings/Button/ResetDownloadDirectory.Content")}
            onClick={() => void handleResetDownloadDirectory()}
          >
            <RotateCcw className="size-4" />
          </Button>
        </SettingsCard>

        <SettingsCard
          icon={KeyRound}
          header={t("Settings/Card/KeychainPassphraseRotation.Header")}
          description={t("Settings/Card/KeychainPassphraseRotation.Description")}
        >
          <Switch
            checked={config.passphraseRotationEnabled}
            onCheckedChange={(checked) => void persist(api.setPassphraseRotation(checked))}
          />
        </SettingsCard>

        <SettingsCard
          icon={RefreshCw}
          header={t("Settings/Card/PurchaseSync.Header")}
          description={t("Settings/Card/PurchaseSync.Description")}
        >
          {syncing ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <span className="text-xs text-muted-foreground">
              {lastSync
                ? t("Settings/PurchaseSync/LastSyncFormat", { 0: new Date(lastSync).toLocaleString() })
                : t("Settings/PurchaseSync/NeverSynced")}
            </span>
          )}
          <Button
            variant="outline"
            size="sm"
            disabled={syncing}
            onClick={() => void handleSync()}
          >
            {t("Settings/PurchaseSync/RefreshButton.Content")}
          </Button>
        </SettingsCard>

        {legacyExists && config?.legacyDbImported !== true && (
          <SettingsCard
            icon={Database}
            header="导入旧版数据"
            description="检测到 WinUI3 版的已购数据库，可导入到本应用。"
          >
            <Button variant="outline" size="sm" onClick={() => void handleLegacyImport()}>
              导入
            </Button>
          </SettingsCard>
        )}

        <SettingsCard
          icon={ExternalLink}
          header={t("Settings/Card/DeveloperSite.Header")}
          description={t("Settings/Card/DeveloperSite.Description")}
        >
          <Button variant="outline" size="sm" onClick={() => void openLink(DEVELOPER_SITE)}>
            {t("Settings/Button/OpenDeveloperSite.Content")}
          </Button>
        </SettingsCard>

        <SettingsCard
          icon={ExternalLink}
          header={t("Settings/Card/ProjectRepository.Header")}
          description={t("Settings/Card/ProjectRepository.Description")}
        >
          <Button variant="outline" size="sm" onClick={() => void openLink(PROJECT_REPO)}>
            {t("Settings/Button/OpenProjectRepository.Content")}
          </Button>
        </SettingsCard>

        <SettingsCard header={t("Settings/Card/AppVersion.Header")}>{version}</SettingsCard>
      </div>

      <CountryPickerDialog
        open={countryOpen}
        onOpenChange={setCountryOpen}
        onPicked={(code) =>
          void persist(
            api.setCountryCode(code),
            t("Settings/CountryCode/UpdatedMessage", { 0: code }),
          )
        }
      />
    </div>
  );
}

function CountryPickerDialog({
  open,
  onOpenChange,
  onPicked,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onPicked: (code: string) => void;
}) {
  const { t } = useTranslation();
  const [items, setItems] = useState<Storefront[]>([]);
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (open && items.length === 0) {
      void api.listStorefronts().then(setItems);
    }
  }, [open, items.length]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter(
      ([code, name]) => code.toLowerCase().includes(q) || name.toLowerCase().includes(q),
    );
  }, [items, query]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t("Settings/CountryCode/DialogTitle")}</DialogTitle>
        </DialogHeader>
        <Input
          placeholder={t("Settings/CountryCode/SearchPlaceholder")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="h-72 overflow-y-auto rounded-md border">
          {filtered.length === 0 ? (
            <p className="p-4 text-sm text-muted-foreground">{t("Settings/CountryCode/NoResults")}</p>
          ) : (
            <ul className="py-1">
              {filtered.map(([code, name]) => (
                <li key={code}>
                  <button
                    className="flex w-full items-center justify-between px-3 py-1.5 text-left text-sm hover:bg-accent"
                    onClick={() => {
                      onPicked(code.toLowerCase());
                      onOpenChange(false);
                    }}
                  >
                    <span>{name}</span>
                    <span className="text-xs text-muted-foreground uppercase">{code}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("Settings/CountryCode/CancelButton")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
