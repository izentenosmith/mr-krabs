import {
  createHashHistory,
  createRootRoute,
  createRoute,
  createRouter,
  type RouterHistory,
} from "@tanstack/react-router";

import { HomePage } from "@/routes/home";
import { RootLayout } from "@/routes/root";

const rootRoute = createRootRoute({ component: RootLayout });

const homeRoute = createRoute({ getParentRoute: () => rootRoute, path: "/", component: HomePage });

const routeTree = rootRoute.addChildren([homeRoute]);

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
