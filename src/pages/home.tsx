import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Copy,
  Download,
  Ellipsis,
  ExternalLink,
  Info,
  Loader2,
  ListFilter,
  ScrollText,
  ShoppingCart,
  Square,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
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
import { SettingsCard } from "@/components/settings-card";
import { api } from "@/lib/api";
import { appStoreUrl, displayStatus, filterResults } from "@/lib/status";
import type { SearchResultItem } from "@/lib/types";
import { useSearch } from "@/stores/search";
import { useQueue } from "@/stores/queue";
import { useSession } from "@/stores/session";
import { useLogs } from "@/stores/logs";
import { useFilter } from "@/stores/filter";
import { cn } from "@/lib/utils";

type Filter = "all" | "not_purchased" | "purchased";

const FILTER_KEY: Record<Filter, string> = {
  all: "MainPage/Filter/AllItem.Content",
  not_purchased: "MainPage/Filter/OnlyNotPurchasedItem.Content",
  purchased: "MainPage/Filter/OnlyPurchasedItem.Content",
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
  // 平台/开发者筛选保存在后端（独立筛选窗口共享），经 filter-changed 同步
  const { platform: platformFilter, developer } = useFilter();

  const [filter, setFilter] = useState<Filter>("all");
  const [busyBundle, setBusyBundle] = useState("");
  const [countryCode, setCountryCode] = useState("cn");

  useEffect(() => {
    void api.getSettings().then((c) => setCountryCode(c.countryCode));
  }, []);

  useEffect(() => {
    void useFilter.getState().init();
  }, []);

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
        <Button variant="outline" size="sm" onClick={() => void api.filterShow()}>
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
  const isBlocked = status === "blocked";

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
                  loading="lazy"
                  decoding="async"
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
                {/* 信息图标：悬停查看不可购买原因；勿用三点（与操作菜单撞形） */}
                <Info className="size-4 text-muted-foreground" />
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
