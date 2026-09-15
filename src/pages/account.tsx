import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, Loader2, RotateCw, LogOut } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { api } from "@/lib/api";
import { useRenderMessage } from "@/lib/messages";
import { useSession } from "@/stores/session";

const APPLE_ACCOUNT_URL = "https://account.apple.com";

export function AccountPage() {
  const { t } = useTranslation();
  const renderMessage = useRenderMessage();
  const { loggedIn, account, isMock, setSession, reset } = useSession();

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [authCode, setAuthCode] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [twoFactorPending, setTwoFactorPending] = useState(false);
  const [busy, setBusy] = useState<"" | "login" | "query" | "logout">("");

  useEffect(() => {
    void api.getPassphrase().then(setPassphrase);
  }, []);

  const isLoggedIn = loggedIn === true;
  const locked = isLoggedIn || busy !== "";

  async function handleLogin() {
    if (!email.trim() || !password.trim()) {
      toast.warning(t("LoginPage/Status/RequiredFieldsEmpty"));
      return;
    }
    setBusy("login");
    try {
      const result = twoFactorPending
        ? await api.verifyAuthCode(email, password, authCode, passphrase)
        : await api.login(email, password, passphrase);

      switch (result.status) {
        case "Success": {
          const mock =
            email.trim().toLowerCase() === "test" && password.trim() === "test";
          setSession(true, email.trim(), mock);
          setTwoFactorPending(false);
          toast.success(t("LoginPage/Status/LoginSuccess"));
          break;
        }
        case "RequiresTwoFactor": {
          setTwoFactorPending(true);
          const detail = renderMessage(result.message);
          toast.warning(detail || t("LoginPage/Status/TwoFactorPromptFallback"));
          break;
        }
        default: {
          toast.error(renderMessage(result.message));
        }
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setBusy("");
    }
  }

  async function handleQuery() {
    setBusy("query");
    try {
      const info = await api.authInfo();
      switch (info.status) {
        case "LoggedIn":
          setSession(true, info.email ?? account ?? "", isMock);
          toast.success(t("LoginPage/Status/AuthInfoSuccess"), {
            description: info.email ?? undefined,
          });
          break;
        case "NotLoggedIn":
          reset();
          setTwoFactorPending(false);
          toast.info(t("LoginPage/Status/AuthInfoNotLoggedIn"));
          break;
        default:
          toast.error(renderMessage(info.message));
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setBusy("");
    }
  }

  async function handleLogout() {
    setBusy("logout");
    try {
      const result = await api.logout();
      if (result.success) {
        reset();
        setTwoFactorPending(false);
        setAuthCode("");
        toast.info(t("LoginPage/Status/LogoutSuccess"));
        if (result.passphraseRotated) {
          void api.getPassphrase().then(setPassphrase);
        }
      } else {
        toast.error(t("LoginPage/Status/LogoutFailed"));
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setBusy("");
    }
  }

  async function handleOpenAppleSite() {
    try {
      await openUrl(APPLE_ACCOUNT_URL);
    } catch (error) {
      toast.error(t("LoginPage/Status/OpenAppleAccountSiteFailed"), {
        description: String(error),
      });
    }
  }

  return (
    <div className="mx-auto w-full max-w-2xl space-y-4 p-6">
      <div>
        <h1 className="text-lg font-semibold">{t("LoginPage/TitleText.Text")}</h1>
        <p className="text-sm text-muted-foreground">{t("LoginPage/SubtitleText.Text")}</p>
      </div>

      <Card className="relative space-y-4 p-5">
        <fieldset disabled={locked} className="space-y-4 disabled:opacity-60">
          <div className="space-y-1.5">
            <Label htmlFor="email">{t("LoginPage/EmailLabel.Text")}</Label>
            <Input
              id="email"
              type="email"
              placeholder={t("LoginPage/EmailInput.PlaceholderText")}
              value={isLoggedIn ? (account ?? "") : email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="password">{t("LoginPage/PasswordLabel.Text")}</Label>
            <Input
              id="password"
              type="password"
              placeholder={t("LoginPage/PasswordInput.PlaceholderText")}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="authCode">
              {t("LoginPage/TwoFactorCodeLabel.Text")}
              {twoFactorPending && <span className="ml-2 text-xs text-amber-600">*</span>}
            </Label>
            <Input
              id="authCode"
              inputMode="numeric"
              placeholder={t("LoginPage/TwoFactorCodeInput.PlaceholderText")}
              value={authCode}
              onChange={(e) => setAuthCode(e.target.value)}
            />
            {twoFactorPending && (
              <p className="text-xs text-muted-foreground">{t("LoginPage/TwoFactorHelpText.Text")}</p>
            )}
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="passphrase">{t("LoginPage/PassphraseLabel.Text")}</Label>
            <Input
              id="passphrase"
              placeholder={t("LoginPage/PassphraseInput.PlaceholderText")}
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
            />
            <p className="text-xs text-muted-foreground">{t("LoginPage/PassphraseChangeHintText.Text")}</p>
          </div>
        </fieldset>

        <div className="flex flex-wrap gap-2 border-t pt-4">
          {!isLoggedIn && (
            <Button onClick={() => void handleLogin()} disabled={busy !== ""}>
              {busy === "login" && <Loader2 className="size-4 animate-spin" />}
              {twoFactorPending ? t("LoginPage/Status/Verifying") : t("LoginPage/LoginButton.Content")}
            </Button>
          )}
          <Button variant="outline" onClick={() => void handleQuery()} disabled={busy !== ""}>
            {busy === "query" ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <RotateCw className="size-4" />
            )}
            {t("LoginPage/QueryAuthInfoButton.Content")}
          </Button>
          {isLoggedIn && (
            <Button variant="outline" onClick={() => void handleLogout()} disabled={busy !== ""}>
              {busy === "logout" ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <LogOut className="size-4" />
              )}
              {t("LoginPage/LogoutButton.Content")}
            </Button>
          )}
          <Button variant="ghost" onClick={() => void handleOpenAppleSite()}>
            <ExternalLink className="size-4" />
            {t("Common/AppleAccount/OpenButton.Content")}
          </Button>
        </div>
      </Card>
    </div>
  );
}
