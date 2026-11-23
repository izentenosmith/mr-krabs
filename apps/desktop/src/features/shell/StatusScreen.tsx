import type { ReactNode } from "react";

/** Full-window holder for app-level states (loading, not yet available, start-up error). */
export function StatusScreen({ children, busy }: { children: ReactNode; busy?: boolean }) {
  return (
    <div aria-busy={busy || undefined} className="bg-background flex min-h-screen items-center justify-center p-8">
      <div className="max-w-md">{children}</div>
    </div>
  );
}
