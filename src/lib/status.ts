import type { AppPlatform, QueueFilter, SearchResultItem } from "./types";

// 前端展示策略（复刻 core purchases/status_policy，仅供渲染；执行仍以后端校验为准）

export type DisplayStatus = "purchased" | "not_purchased" | "blocked";

export function isFreePrice(price: string | undefined | null): boolean {
  if (!price) return false;
  const trimmed = price.trim();
  if (!trimmed) return false;
  const number = Number(trimmed);
  if (!Number.isNaN(number)) return number <= 0;
  return trimmed.toLowerCase() === "free";
}

export function displayPrice(price: string): string {
  return isFreePrice(price) ? "free" : price;
}

// 购买状态 token 与 core status_policy 对齐：purchased / not_purchased / blocked
export function displayStatus(item: SearchResultItem): DisplayStatus {
  if (item.purchased === "purchased") return "purchased";
  if (item.purchased === "blocked") return "blocked";
  return "not_purchased";
}

// 队列状态 → i18n 键
export function queueStatusKey(status: string): string {
  return `DownloadQueue/Status/${status}`;
}

// 日志等级 → tag 与颜色（对齐旧版：深色底，INFO 浅灰）
export function logLevelTag(level: string): string {
  switch (level) {
    case "tip":
      return "TIP";
    case "success":
      return "SUCCESS";
    case "error":
      return "ERROR";
    case "ipatool":
      return "ipatool";
    default:
      return "INFO";
  }
}

export const LOG_LEVEL_CLASS: Record<string, string> = {
  info: "text-zinc-300",
  tip: "text-sky-300",
  success: "text-green-400",
  error: "text-red-400",
  ipatool: "text-amber-300",
};

export function appStoreUrl(countryCode: string, appId: string): string {
  return `https://apps.apple.com/${countryCode}/app/id${appId}`;
}

/** 开发者下拉选项：去空白、大小写不敏感去重、保持出现顺序。 */
export function collectDevelopers(results: SearchResultItem[]): string[] {
  const options: string[] = [];
  for (const item of results) {
    const name = item.developer?.trim();
    if (!name) continue;
    if (!options.some((o) => o.toLowerCase() === name.toLowerCase())) options.push(name);
  }
  return options;
}

/** 平台筛选：全部 / iOS / Mac。 */
export type PlatformFilter = "all" | AppPlatform;

/** 筛选：全部 / 未购买 / 已购买 + 开发者（大小写不敏感）+ 平台。 */
export function filterResults(
  results: SearchResultItem[],
  filter: QueueFilter,
  developer: string,
  platform: PlatformFilter = "all",
): SearchResultItem[] {
  return results.filter((item) => {
    const status = displayStatus(item);
    if (filter === "purchased" && status !== "purchased") return false;
    if (filter === "not_purchased" && status === "purchased") return false;
    if (developer !== "all" && item.developer?.trim().toLowerCase() !== developer.toLowerCase())
      return false;
    if (platform !== "all" && item.platform !== platform) return false;
    return true;
  });
}
