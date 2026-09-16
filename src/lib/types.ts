//! 后端 DTO 的 TS 镜像（serde 序列化均为 camelCase）。

export interface JsMessageKey {
  type: "key";
  key: string;
  args: string[];
}

export interface JsMessageRaw {
  type: "raw";
  text: string;
}

export type JsMessage = JsMessageKey | JsMessageRaw;

export interface AppConfig {
  countryCode: string;
  downloadDirectory: string | null;
  displayLanguage: "auto" | "zh-Hans" | "en-US";
  detailedIpatoolLog: boolean;
  passphraseRotationEnabled: boolean;
  ipatoolFlavor: "main" | "custom";
  customIpatoolPath: string | null;
  legacyDbImported: boolean;
}

export type AuthStatus =
  | "Success"
  | "RequiresTwoFactor"
  | "InvalidCredential"
  | "AuthCodeInvalid"
  | "NetworkError"
  | "Timeout"
  | "UnknownError";

export interface AuthResult {
  status: AuthStatus;
  message: JsMessage;
  rawPayload: string | null;
}

export interface AuthInfo {
  status: "LoggedIn" | "NotLoggedIn" | "Error";
  email: string | null;
  message: JsMessage | null;
}

export interface LogoutResult {
  success: boolean;
  passphraseRotated: boolean;
  message: JsMessage | null;
}

export type Storefront = readonly [code: string, name: string];

export type PurchaseStatus = "purchased" | "not_purchased";

/** App Store 平台：iOS（缺省）、iPadOS 与 Mac App Store。 */
export type AppPlatform = "ios" | "ipad" | "macos";

export interface SearchResultItem {
  bundleId: string;
  id: string | null;
  name: string | null;
  developer: string | null;
  artworkUrl: string | null;
  price: string;
  version: string | null;
  platform: AppPlatform;
  purchased: string;
}

export type PurchaseOutcome =
  | "Purchased"
  | "AlreadyOwned"
  | "NeedsOwnedConfirmation"
  | "Skipped"
  | "Failed";

export interface PurchaseResult {
  bundleId: string;
  outcome: PurchaseOutcome;
  detail: string | null;
}

export type QueueItemStatus = "Pending" | "Downloading" | "Success" | "Failed" | "Canceled";

export interface QueueItem {
  bundleId: string;
  platform: AppPlatform;
  appId: string;
  name: string;
  developer: string;
  version: string;
  price: string;
  artworkUrl: string;
  status: QueueItemStatus;
  lastMessage: string;
}

export interface QueueStatus {
  running: boolean;
  items: QueueItem[];
}

export interface LogEntry {
  timestamp: string;
  level: "info" | "tip" | "success" | "error" | "ipatool";
  message: JsMessage;
}

export type QueueFilter = "all" | "not_purchased" | "purchased";

/** 主页筛选（筛选窗口与主窗口共享；developers 为当前搜索的开发者选项）。 */
export interface FilterSelection {
  platform: "all" | AppPlatform;
  developer: string;
  developers: string[];
}
