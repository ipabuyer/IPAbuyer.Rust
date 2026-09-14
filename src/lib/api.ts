import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, AuthInfo, AuthResult, LogoutResult, Storefront } from "./types";

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
