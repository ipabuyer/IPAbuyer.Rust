import { useTranslation } from "react-i18next";

export function HomePage() {
  const { t } = useTranslation();
  return (
    <div className="p-6">
      <h1 className="text-lg font-semibold">{t("MainWindow/Nav/Main.Content")}</h1>
    </div>
  );
}
