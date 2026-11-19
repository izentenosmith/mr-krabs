import { Outlet } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { Toaster } from "@/components/ui/sonner";

export function RootLayout() {
  const { t } = useTranslation();
  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col gap-6 p-8">
      <header className="flex items-center gap-3">
        <img src="/favicon.png" alt="" className="size-8" />
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Mr Krabs</h1>
          <p className="text-muted-foreground text-sm">{t("app.tagline")}</p>
        </div>
      </header>
      <Outlet />
      <Toaster />
    </main>
  );
}
