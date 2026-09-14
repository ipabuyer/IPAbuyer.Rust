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
