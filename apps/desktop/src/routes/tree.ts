import { createRootRoute, createRoute } from "@tanstack/react-router";

import { AppLayout } from "@/routes/app-layout";
import { RootLayout } from "@/routes/root";

export const rootRoute = createRootRoute({ component: RootLayout });

/**
 * Pathless layout route: sidebar + content outlet (MASTER §5). Every screen of the app is
 * a child of this route — create it with `getParentRoute: () => appLayoutRoute` and list it
 * in `appChildRoutes` (router.tsx).
 */
export const appLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "app",
  component: AppLayout,
});
