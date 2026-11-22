import { Link, Outlet } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { AccountsSidebarContainer } from "@/features/accounts/AccountsSidebarContainer";
import { navItems } from "@/routes/nav";

/** App shell: 240px sidebar (brand, nav, accounts) and the content outlet (MASTER §5). */
export function AppLayout() {
  const { t } = useTranslation();
  return (
    <div className="bg-background text-foreground flex min-h-screen">
      <aside className="bg-muted z-20 flex w-60 shrink-0 flex-col gap-6 border-r p-3">
        <div className="flex items-center gap-2 px-2 pt-1">
          <img src="/favicon.png" alt="" className="size-5" />
          <span className="text-sm font-semibold">{t("shell.brand")}</span>
        </div>
        <nav aria-label={t("shell.nav")}>
          <ul className="flex flex-col gap-0.5">
            {navItems.map((item) => (
              <li key={item.labelKey}>
                <Link
                  to={item.to}
                  className="hover:bg-accent flex h-8 items-center gap-2 rounded-md border-l-[3px] border-transparent px-2 text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  activeProps={{ className: "bg-accent border-brand font-medium" }}
                >
                  <item.icon className="size-4" aria-hidden />
                  {t(item.labelKey)}
                </Link>
              </li>
            ))}
          </ul>
        </nav>
        <AccountsSidebarContainer />
      </aside>
      <main className="flex min-w-0 flex-1 flex-col gap-6 p-8">
        <Outlet />
      </main>
    </div>
  );
}
