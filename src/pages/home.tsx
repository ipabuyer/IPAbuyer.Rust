import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Copy,
  Download,
  Ellipsis,
  ExternalLink,
  Loader2,
  ShoppingCart,
  Square,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
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
import { SettingsCard } from "@/components/settings-card";
import { api } from "@/lib/api";
import { appStoreUrl, displayStatus } from "@/lib/status";
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
  const [busyBundle, setBusyBundle] = useState("");
  const [countryCode, setCountryCode] = useState("cn");

  useEffect(() => {
    void api.getSettings().then((c) => setCountryCode(c.countryCode));
  }, []);

  const developers = useMemo(() => {
    const options: string[] = [];
    for (const item of results) {
      const name = item.developer?.trim();
      if (!name) continue;
      if (!options.some((o) => o.toLowerCase() === name.toLowerCase())) options.push(name);
    }
    return options;
  }, [results]);

  const filtered = useMemo(() => {
    return results.filter((item) => {
      const status = displayStatus(item);
      if (filter === "purchased" && status !== "purchased") return false;
      if (filter === "not_purchased" && status === "purchased") return false;
      if (developer !== "all" && item.developer?.trim().toLowerCase() !== developer.toLowerCase())
        return false;
      return true;
    });
  }, [results, filter, developer]);

  function requireLogin(): boolean {
    if (loggedIn === true) return true;
    toast.warning(t("MainPage/Purchase/LoginRequired"));
    return false;
  }

  async function handlePurchase(item: SearchResultItem) {
    if (!requireLogin()) return;
    setBusyBundle(item.bundleId);
    try {
      const result = await api.purchase(item.bundleId, item.price, item.purchased);
      switch (result.outcome) {
        case "Purchased":
          toast.success(result.detail === "Mock" ? t("MainPage/Purchase/MockSuccess") : t("MainPage/Purchase/Success"));
          markLocal(item.bundleId, "purchased");
          break;
        case "AlreadyOwned":
        case "NeedsOwnedConfirmation":
          toast.success(t("MainPage/Purchase/OwnedDetected"));
          markLocal(item.bundleId, "purchased");
          break;
        case "Skipped":
          toast.info(t("MainPage/Purchase/SkipNonFree"));
          break;
        default:
          toast.error(t("MainPage/Purchase/Failed"), { description: result.detail ?? undefined });
      }
    } catch (error) {
      toast.error(t("MainPage/Purchase/Failed"), { description: String(error) });
    } finally {
      setBusyBundle("");
    }
  }

  async function handleDownload(item: SearchResultItem) {
    if (!requireLogin()) return;
    const added = await api.queueAdd({
      bundleId: item.bundleId,
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
        toast.error(t("MainPage/DownloadQueue/StartFailed"), { description: String(error) });
        void refresh();
        return;
      }
    }
    void refresh();
  }

  function markLocal(bundleId: string, status: string) {
    useSearch.setState((s) => ({
      results: s.results.map((r) =>
        r.bundleId.toLowerCase() === bundleId.toLowerCase() ? { ...r, purchased: status } : r,
      ),
    }));
  }

  async function handleMark(item: SearchResultItem, status: string) {
    try {
      await api.mark(item.bundleId, status);
      markLocal(item.bundleId, status);
    } catch (error) {
      toast.error(String(error));
    }
  }

  async function handleUnmark(item: SearchResultItem) {
    try {
      await api.unmark(item.bundleId);
      markLocal(item.bundleId, "not_purchased");
    } catch (error) {
      toast.error(String(error));
    }
  }

  async function handleCopy(text: string | null) {
    if (!text) {
      toast.info(t("MainPage/Log/CopyFieldEmpty"));
      return;
    }
    await navigator.clipboard.writeText(text);
    toast.success(t("MainPage/Log/CopyFieldSuccess"));
  }

  async function handleOpenAppStore(item: SearchResultItem) {
    if (!item.id) {
      toast.warning(t("MainPage/Log/AppStoreMissingId"));
      return;
    }
    try {
      await openUrl(appStoreUrl(countryCode, item.id));
    } catch (error) {
      toast.error(t("MainPage/Log/AppStoreOpenFailed"), { description: String(error) });
    }
  }

  const downloading = running || items.some((i) => i.status === "Downloading");

  return (
    <div className="flex h-full flex-col">
      {/* 筛选/操作区 */}
      <div className="flex flex-wrap items-center gap-2 border-b px-6 py-3">
        <div className="flex overflow-hidden rounded-md border">
          {(["all", "not_purchased", "purchased"] as Filter[]).map((key) => (
            <button
              key={key}
              className={cn(
                "px-3 py-1.5 text-xs",
                filter === key ? "bg-primary text-primary-foreground" : "hover:bg-accent",
              )}
              onClick={() => setFilter(key)}
            >
              {t(FILTER_KEY[key])}
            </button>
          ))}
        </div>
        <Select value={developer} onValueChange={setDeveloper}>
          <SelectTrigger className="h-8 w-44 text-xs">
            <SelectValue placeholder={t("MainPage/DeveloperSelectorAllItem.Content")} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t("MainPage/DeveloperSelectorAllItem.Content")}</SelectItem>
            {developers.map((name) => (
              <SelectItem key={name} value={name}>
                {name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="flex-1" />
        {downloading && <ActivityRing />}
        {downloading && (
          <Button variant="outline" size="sm" onClick={() => api.queueCancel()}>
            <Square className="size-4" />
            {t("MainPage/Action/CancelAllDownloadsButton.Content")}
          </Button>
        )}
        <Button variant="outline" size="sm" onClick={() => openLog(true)}>
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
                key={item.bundleId}
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
  onCopy: (text: string | null) => Promise<void>;
  onOpenAppStore: () => void;
}) {
  const { t } = useTranslation();
  const status = displayStatus(item);
  const queueItem = useQueue((s) =>
    s.items.find((i) => i.bundleId.toLowerCase() === item.bundleId.toLowerCase()),
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

  return (
    <SettingsCard
      header={item.name ?? item.bundleId}
      description={item.developer ?? ""}
      className={cn(isBlocked && "opacity-90")}
    >
      {item.artworkUrl && (
        <img src={item.artworkUrl} alt="" className="size-12 rounded-lg object-cover" />
      )}
      <div className="flex flex-col items-end gap-1">
        <span className="text-xs text-muted-foreground">{item.version}</span>
        <span className={cn("text-xs font-medium", statusClass)}>{statusText}</span>
      </div>
      {isBlocked && (
        <span title={t("MainPage/PurchaseBlockedReason/NonFree")}>
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
          {isPurchased ? (
            <DropdownMenuItem onClick={onUnmark}>
              {t("MainPage/Context/MarkNotPurchasedItem.Text")}
            </DropdownMenuItem>
          ) : (
            <DropdownMenuItem onClick={onMark}>
              {t("MainPage/Context/MarkPurchasedItem.Text")}
            </DropdownMenuItem>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => void onCopy(item.name)}>
            <Copy className="size-4" />
            {t("MainPage/Context/CopyNameItem.Text")}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => void onCopy(item.bundleId)}>
            <Copy className="size-4" />
            {t("MainPage/Context/CopyIdItem.Text")}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={onOpenAppStore}>
            <ExternalLink className="size-4" />
            {t("MainPage/Context/OpenAppStoreItem.Text")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </SettingsCard>
  );
}
