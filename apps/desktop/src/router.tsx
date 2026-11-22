import { createHashHistory, createRoute, createRouter, type RouterHistory } from "@tanstack/react-router";

import { HomePage } from "@/routes/home";
import { transactionsRoute } from "@/routes/transactions";
import { appLayoutRoute, rootRoute } from "@/routes/tree";

const homeRoute = createRoute({ getParentRoute: () => appLayoutRoute, path: "/", component: HomePage });

/**
 * Screens inside the app shell. ui-transactions (S7) adds its route here; its nav item goes
 * in `navItems` (routes/nav.ts).
 */
const appChildRoutes = [homeRoute, transactionsRoute];

const routeTree = rootRoute.addChildren([appLayoutRoute.addChildren(appChildRoutes)]);

/**
 * Hash history by default: the Tauri webview and the Vite dev server then serve the same
 * index.html for every route without any fallback configuration.
 */
export function createAppRouter(history: RouterHistory = createHashHistory()) {
  return createRouter({ routeTree, history });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
