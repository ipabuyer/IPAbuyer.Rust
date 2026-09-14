import { useTranslation } from "react-i18next";

export function AccountPage() {
  const { t } = useTranslation();
  return (
    <div className="p-6">
      <h1 className="text-lg font-semibold">{t("MainWindow/Nav/Account.Content")}</h1>
    </div>
  );
}
