import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Copy,
  Download,
  Ellipsis,
  ExternalLink,
  Loader2,
  ListFilter,
  ScrollText,
  ShoppingCart,
  Square,
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
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Label } from "@/components/ui/label";
import { SettingsCard } from "@/components/settings-card";
import { api } from "@/lib/api";
import {
  appStoreUrl,
  collectDevelopers,
  displayStatus,
  filterResults,
  type PlatformFilter,
} from "@/lib/status";
import type { SearchResultItem } from "@/lib/types";
import { useSearch } from "@/stores/search";
import { useQueue } from "@/stores/queue";
import { useSession } from "@/stores/session";
import { useLogs } from "@/stores/logs";
import { cn } from "@/lib/utils";

type Filter = "all" | "not_purchased" | "purchased";

const FILTER_KEY: Record<Filter, string> = {
  all: "MainPage/Filter/AllItem.Content",
  not_purchased: "MainPage/Filter/OnlyNotPurchasedItem.Content",
  purchased: "MainPage/Filter/OnlyPurchasedItem.Content",
};

const PLATFORM_KEY: Record<PlatformFilter, string> = {
  all: "MainPage/Filter/AllPlatforms",
  ios: "MainPage/Platform/Ios",
  ipad: "MainPage/Platform/Ipad",
  macos: "MainPage/Platform/Macos",
};

/** 非 iOS 平台的卡片徽标键（iOS 为缺省平台不显示徽标）。 */
function platformBadgeKey(platform: SearchResultItem["platform"]): string | null {
  if (platform === "macos") return "MainPage/Platform/Macos";
  if (platform === "ipad") return "MainPage/Platform/Ipad";
  return null;
}

// 应用显示名：名称为空白时回退 bundleId（对齐 WinUI3 GetAppDisplayLabel）
function appDisplayLabel(item: SearchResultItem): string {
  return item.name?.trim() ? item.name : item.bundleId;
}

// 下载进度环（不确定式，对应 WinUI ActivityRing）
function ActivityRing() {
  return (
    <svg className="size-5 animate-spin text-primary" viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="3" className="opacity-20" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}

export function HomePage() {
  const { t } = useTranslation();
  const { query, results, searching } = useSearch();
  const { running, items, refresh } = useQueue();
  const loggedIn = useSession((s) => s.loggedIn);
  const openLog = useLogs((s) => s.setOpen);

  const [filter, setFilter] = useState<Filter>("all");
  const [developer, setDeveloper] = useState("all");
  const [platformFilter, setPlatformFilter] = useState<PlatformFilter>("all");
  const [filterOpen, setFilterOpen] = useState(false);
  const [busyBundle, setBusyBundle] = useState("");
  const [countryCode, setCountryCode] = useState("cn");

  useEffect(() => {
    void api.getSettings().then((c) => setCountryCode(c.countryCode));
  }, []);

  const developers = useMemo(() => collectDevelopers(results), [results]);

  // 搜索结果变化后，已选开发者不在新结果中时回退为全部开发者
  useEffect(() => {
    if (developer === "all") return;
    if (!developers.some((d) => d.toLowerCase() === developer.toLowerCase())) {
      setDeveloper("all");
    }
  }, [developers, developer]);

  const filtered = useMemo(
    () => filterResults(results, filter, developer, platformFilter),
    [results, filter, developer, platformFilter],
  );

  function requireLogin(): boolean {
    if (loggedIn === true) return true;
    toast.warning(t("MainPage/Purchase/LoginRequired"));
    return false;
  }

  async function handlePurchase(item: SearchResultItem) {
    if (!requireLogin()) return;
    setBusyBundle(item.bundleId);
    const label = appDisplayLabel(item);
    try {
      const result = await api.purchase(
        item.bundleId,
        item.price,
        item.purchased,
        item.platform,
      );
      switch (result.outcome) {
        case "Purchased":
          toast.success(
            result.detail === "Mock"
              ? t("MainPage/Purchase/MockSuccess", { 0: label })
              : t("MainPage/Purchase/Success", { 0: label }),
          );
          markLocal(item, "purchased");
          break;
        case "AlreadyOwned":
        case "NeedsOwnedConfirmation":
          toast.success(t("MainPage/Purchase/OwnedDetected", { 0: label }));
          markLocal(item, "purchased");
          break;
        case "Skipped":
          toast.info(t("MainPage/Purchase/SkipNonFree", { 0: label }));
          break;
        default:
          toast.error(
            t("MainPage/Purchase/Failed", {
              0: label,
              1: result.detail?.trim() || t("MainPage/Purchase/UnknownError"),
            }),
          );
      }
    } catch (error) {
      toast.error(t("MainPage/Purchase/Failed", { 0: label, 1: String(error) }));
    } finally {
      setBusyBundle("");
    }
  }

  async function handleDownload(item: SearchResultItem) {
    if (!requireLogin()) return;
    // 下载的可见反馈少（进度环不明显），自动展开日志窗口展示队列过程
    openLog(true);
    const added = await api.queueAdd({
      bundleId: item.bundleId,
      platform: item.platform,
      appId: item.id,
      name: item.name,
      developer: item.developer,
      version: item.version,
      price: item.price,
      artworkUrl: item.artworkUrl,
    });
    if (added === "Ignored") {
      toast.info(t("MainPage/DownloadQueue/AddContextIgnored"));
      return;
    }
    if (!useQueue.getState().running) {
      try {
        await api.queueStart();
      } catch (error) {
        toast.error(t("MainPage/DownloadQueue/StartFailed", { 0: String(error) }));
        void refresh();
        return;
      }
    }
    void refresh();
  }

  function markLocal(item: SearchResultItem, status: string) {
    useSearch.setState((s) => ({
      results: s.results.map((r) =>
        r.bundleId.toLowerCase() === item.bundleId.toLowerCase() && r.platform === item.platform
          ? { ...r, purchased: status }
          : r,
      ),
    }));
  }

  async function handleMark(item: SearchResultItem, status: string) {
    try {
      await api.mark(item.bundleId, status, item.platform);
      markLocal(item, status);
    } catch (error) {
      toast.error(String(error));
    }
  }

  async function handleUnmark(item: SearchResultItem) {
    try {
      await api.unmark(item.bundleId, item.platform);
      markLocal(item, "not_purchased");
    } catch (error) {
      toast.error(String(error));
    }
  }

  async function handleCopy(text: string | null, field: "name" | "id") {
    const fieldLabel = t(field === "name" ? "MainPage/Field/Name" : "MainPage/Field/Id");
    if (!text?.trim()) {
      toast.info(t("MainPage/Log/CopyFieldEmpty", { 0: fieldLabel }));
      return;
    }
    await navigator.clipboard.writeText(text);
    toast.success(t("MainPage/Log/CopyFieldSuccess", { 0: fieldLabel, 1: 1 }));
  }

  async function handleOpenAppStore(item: SearchResultItem) {
    if (!item.id) {
      toast.warning(t("MainPage/Log/AppStoreMissingId"));
      return;
    }
    try {
      await openUrl(appStoreUrl(countryCode, item.id));
    } catch (error) {
      toast.error(t("MainPage/Log/AppStoreOpenFailed", { 0: String(error) }));
    }
  }

  const downloading = running || items.some((i) => i.status === "Downloading");

  return (
    <div className="flex h-full flex-col">
      {/* 筛选/操作区 */}
      <div className="flex flex-wrap items-center gap-2 border-b px-6 py-3">
        <div className="flex h-8 overflow-hidden rounded-md border">
          {(["all", "not_purchased", "purchased"] as Filter[]).map((key) => (
            <button
              key={key}
              className={cn(
                "h-full px-3 text-xs",
                filter === key ? "bg-primary text-primary-foreground" : "hover:bg-accent",
              )}
              onClick={() => setFilter(key)}
            >
              {t(FILTER_KEY[key])}
            </button>
          ))}
        </div>
        <div className="flex-1" />
        {downloading && <ActivityRing />}
        {downloading && (
          <Button variant="outline" size="sm" onClick={() => api.queueCancel()}>
            <Square className="size-4" />
            {t("MainPage/Action/CancelAllDownloadsButton.Content")}
          </Button>
        )}
        <Button variant="outline" size="sm" onClick={() => setFilterOpen(true)}>
          <ListFilter className="size-4" />
          {t("MainPage/Action/FilterButton.Content")}
        </Button>
        <Button variant="outline" size="sm" onClick={() => openLog(true)}>
          <ScrollText className="size-4" />
          {t("MainPage/Action/OpenLogButton.Content")}
        </Button>
      </div>

      {/* 结果列表 */}
      <div className="flex-1 overflow-y-auto px-6 py-3">
        {searching ? (
          <div className="flex justify-center py-16">
            <Loader2 className="size-6 animate-spin text-muted-foreground" />
          </div>
        ) : results.length === 0 ? (
          <p className="py-16 text-center text-sm text-muted-foreground">
            {query ? t("MainPage/EmptySearchHint/SearchEmpty") : t("MainPage/EmptySearchHint/Initial")}
          </p>
        ) : filtered.length === 0 ? (
          <p className="py-16 text-center text-sm text-muted-foreground">
            {t("MainPage/EmptySearchHint/FilterEmpty")}
          </p>
        ) : (
          <div className="space-y-2">
            {filtered.map((item) => (
              <AppCard
                key={`${item.platform}:${item.bundleId}`}
                item={item}
                busy={busyBundle === item.bundleId}
                onPurchase={() => void handlePurchase(item)}
                onDownload={() => void handleDownload(item)}
                onMark={() => void handleMark(item, "purchased")}
                onUnmark={() => void handleUnmark(item)}
                onCopy={handleCopy}
                onOpenAppStore={() => void handleOpenAppStore(item)}
              />
            ))}
          </div>
        )}
      </div>

      {/* 筛选弹窗：平台 + 开发者（条件即时生效） */}
      <Dialog open={filterOpen} onOpenChange={setFilterOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("MainPage/Action/FilterButton.Content")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">
                {t("MainPage/Filter/Platform")}
              </Label>
              <div className="flex h-8 w-fit overflow-hidden rounded-md border">
                {(["all", "ios", "ipad", "macos"] as PlatformFilter[]).map((key) => (
                  <button
                    key={key}
                    className={cn(
                      "h-full px-3 text-xs",
                      platformFilter === key
                        ? "bg-primary text-primary-foreground"
                        : "hover:bg-accent",
                    )}
                    onClick={() => setPlatformFilter(key)}
                  >
                    {t(PLATFORM_KEY[key])}
                  </button>
                ))}
              </div>
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">
                {t("MainPage/Filter/Developer")}
              </Label>
              <Select value={developer} onValueChange={setDeveloper}>
                <SelectTrigger size="sm" className="w-full text-xs">
                  <SelectValue placeholder={t("MainPage/DeveloperSelectorAllItem.Content")} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">
                    {t("MainPage/DeveloperSelectorAllItem.Content")}
                  </SelectItem>
                  {developers.map((name) => (
                    <SelectItem key={name} value={name}>
                      {name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setFilterOpen(false)}>
              {t("Settings/CountryCode/CancelButton")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function AppCard({
  item,
  busy,
  onPurchase,
  onDownload,
  onMark,
  onUnmark,
  onCopy,
  onOpenAppStore,
}: {
  item: SearchResultItem;
  busy: boolean;
  onPurchase: () => void;
  onDownload: () => void;
  onMark: () => void;
  onUnmark: () => void;
  onCopy: (text: string | null, field: "name" | "id") => Promise<void>;
  onOpenAppStore: () => void;
}) {
  const { t } = useTranslation();
  const status = displayStatus(item);
  const queueItem = useQueue((s) =>
    s.items.find(
      (i) =>
        i.bundleId.toLowerCase() === item.bundleId.toLowerCase() && i.platform === item.platform,
    ),
  );
  const isPurchased = status === "purchased";
  const isBlocked = status === "purchase_blocked";

  const statusText = queueItem
    ? t(`DownloadQueue/Status/${queueItem.status}`)
    : isPurchased
      ? t("Common/Status/Purchased")
      : isBlocked
        ? t("Common/Status/PurchaseBlocked")
        : t("Common/Status/CanPurchase");
  const statusClass = queueItem
    ? queueItem.status === "Success"
      ? "text-green-600"
      : queueItem.status === "Failed" || queueItem.status === "Canceled"
        ? "text-red-500"
        : "text-primary"
    : isPurchased
      ? "text-green-600"
      : isBlocked
        ? "text-red-500"
        : "text-muted-foreground";

  // 三点按钮（DropdownMenu）与卡片右键（ContextMenu）渲染同一组菜单项
  function menuItems(
    Item: React.ComponentType<{
      onClick?: () => void;
      className?: string;
      children?: React.ReactNode;
    }>,
    Separator: React.ComponentType<{ className?: string }>,
  ) {
    return (
      <>
        {isPurchased ? (
          <Item onClick={onUnmark}>
            {t("MainPage/Context/MarkNotPurchasedItem.Text")}
          </Item>
        ) : (
          <Item onClick={onMark}>{t("MainPage/Context/MarkPurchasedItem.Text")}</Item>
        )}
        <Separator />
        <Item onClick={() => void onCopy(item.name, "name")}>
          <Copy className="size-4" />
          {t("MainPage/Context/CopyNameItem.Text")}
        </Item>
        <Item onClick={() => void onCopy(item.bundleId, "id")}>
          <Copy className="size-4" />
          {t("MainPage/Context/CopyIdItem.Text")}
        </Item>
        <Separator />
        <Item onClick={onOpenAppStore}>
          <ExternalLink className="size-4" />
          {t("MainPage/Context/OpenAppStoreItem.Text")}
        </Item>
      </>
    );
  }

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div>
          <SettingsCard
            header={item.name ?? item.bundleId}
            description={item.developer ?? ""}
            className={cn(isBlocked && "opacity-90")}
            image={
              item.artworkUrl ? (
                <img
                  src={item.artworkUrl}
                  alt=""
                  className="size-12 shrink-0 rounded-lg object-cover"
                />
              ) : undefined
            }
          >
            {/* WinUI3 卡片架构：版本号/状态为标题与动作区之间的独立横向列 */}
            {(() => {
              const badgeKey = platformBadgeKey(item.platform);
              return badgeKey ? (
                <span
                  title={t(badgeKey)}
                  className="shrink-0 rounded border border-border px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground"
                >
                  {t(badgeKey)}
                </span>
              ) : null;
            })()}
            <span
              className="w-20 shrink-0 text-right text-xs text-muted-foreground"
              title={item.version ?? undefined}
            >
              {item.version}
            </span>
            <span
              className={cn(
                "w-24 shrink-0 truncate text-right text-xs font-medium",
                statusClass,
              )}
              title={statusText}
            >
              {statusText}
            </span>
            {isBlocked && (
              <span
                title={
                  item.price.trim()
                    ? t("MainPage/PurchaseBlockedReason/NonFree", { 0: item.price.trim() })
                    : t("MainPage/PurchaseBlockedReason/Unknown")
                }
              >
                <Ellipsis className="size-4 text-muted-foreground" />
              </span>
            )}
            {!isBlocked && (
              <Button
                size="sm"
                variant={isPurchased ? "outline" : "default"}
                disabled={busy || queueItem?.status === "Downloading"}
                onClick={isPurchased ? onDownload : onPurchase}
              >
                {busy ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : isPurchased ? (
                  <Download className="size-4" />
                ) : (
                  <ShoppingCart className="size-4" />
                )}
                {isPurchased
                  ? t("MainPage/Action/AddToQueueButton.Content")
                  : t("MainPage/Context/PurchaseItem.Text")}
              </Button>
            )}
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="icon" className="size-8">
                  <Ellipsis className="size-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                {menuItems(DropdownMenuItem, DropdownMenuSeparator)}
              </DropdownMenuContent>
            </DropdownMenu>
          </SettingsCard>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent>{menuItems(ContextMenuItem, ContextMenuSeparator)}</ContextMenuContent>
    </ContextMenu>
  );
}
