import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AppPlatform,
  AuthInfo,
  AuthResult,
  LogoutResult,
  PurchaseResult,
  QueueStatus,
  SearchResultItem,
  Storefront,
} from "./types";

export const api = {
  // ---- 设置 ----
  getSettings: () => invoke<AppConfig>("settings_get"),
  defaultDownloadDirectory: () => invoke<string>("settings_default_download_directory"),
  setCountryCode: (code: string) => invoke<AppConfig>("settings_set_country_code", { code }),
  setDownloadDirectory: (path: string) =>
    invoke<AppConfig>("settings_set_download_directory", { path }),
  resetDownloadDirectory: () => invoke<AppConfig>("settings_reset_download_directory"),
  setDisplayLanguage: (language: string) =>
    invoke<AppConfig>("settings_set_display_language", { language }),
  setDetailedLog: (enabled: boolean) =>
    invoke<AppConfig>("settings_set_detailed_log", { enabled }),
  setPassphraseRotation: (enabled: boolean) =>
    invoke<AppConfig>("settings_set_passphrase_rotation", { enabled }),
  getPassphrase: () => invoke<string>("settings_get_passphrase"),
  listStorefronts: () => invoke<Storefront[]>("settings_list_storefronts"),

  // ---- 搜索 / 购买 / 队列 / 日志 ----
  search: (query: string) => invoke<SearchResultItem[]>("catalog_search", { query }),
  purchase: (bundleId: string, price: string, purchased: string, platform: AppPlatform) =>
    invoke<PurchaseResult>("purchase", { bundleId, price, purchased, platform }),
  mark: (bundleId: string, status: string, platform: AppPlatform) =>
    invoke<void>("purchases_mark", { bundleId, status, platform }),
  unmark: (bundleId: string, platform: AppPlatform) =>
    invoke<void>("purchases_unmark", { bundleId, platform }),
  queueAdd: (item: {
    bundleId: string;
    platform: AppPlatform;
    appId: string | null;
    name: string | null;
    developer: string | null;
    version: string | null;
    price: string;
    artworkUrl: string | null;
  }) => invoke<"Added" | "Updated" | "Requeued" | "Ignored">("queue_add", item),
  queueStart: () => invoke<void>("queue_start"),
  queueStatus: () => invoke<QueueStatus>("queue_status"),
  queueCancel: () => invoke<void>("queue_cancel_current"),
  logsSnapshot: () => {
    return invoke<import("./types").LogEntry[]>("logs_snapshot");
  },
  logsClear: () => invoke<void>("logs_clear"),

  // ---- 同步 ----
  syncStart: () =>
    invoke<{ outcome: string; synced: number; total: number; message: string | null }>(
      "sync_start",
    ),
  syncCancel: () => invoke<void>("sync_cancel"),
  syncStatus: () => invoke<{ running: boolean; synced: number; total: number }>("sync_status"),
  syncLastTime: () => invoke<string | null>("sync_last_time"),

  // ---- ipatool 管理 ----
  ipatoolInfo: () =>
    invoke<{
      flavor: "main" | "custom";
      customPath: string | null;
      builtinVersion: string;
      activePath: string;
      builtinAvailable: boolean;
      dataDirectory: string;
    }>("ipatool_info"),
  ipatoolSetFlavor: (flavor: string) => invoke<void>("ipatool_set_flavor", { flavor }),
  ipatoolSetCustomPath: (path: string) => invoke<void>("ipatool_set_custom_path", { path }),
  ipatoolDeleteCustom: () => invoke<void>("ipatool_delete_custom"),
  ipatoolExport: () => invoke<string>("ipatool_export"),
  ipatoolClearData: () => invoke<void>("ipatool_clear_data"),
  legacyDbExists: () => invoke<boolean>("legacy_db_exists"),
  legacyDbImport: () => invoke<void>("legacy_db_import"),

  // ---- 认证 ----
  login: (account: string, password: string, passphrase: string) =>
    invoke<AuthResult>("auth_login", { account, password, passphrase: nullable(passphrase) }),
  verifyAuthCode: (account: string, password: string, authCode: string, passphrase: string) =>
    invoke<AuthResult>("auth_verify_code", {
      account,
      password,
      authCode,
      passphrase: nullable(passphrase),
    }),
  logout: () => invoke<LogoutResult>("auth_logout"),
  authInfo: () => invoke<AuthInfo>("auth_info"),
};

function nullable(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}
