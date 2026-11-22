import type { ReactNode } from "react";
import { CircleAlertIcon } from "lucide-react";

/** An error the user should read: CircleAlert + destructive text (MASTER §7.1, DEC-12). */
export function ErrorAlert({ children }: { children: ReactNode }) {
  return (
    <div role="alert" className="text-destructive flex items-start gap-2 text-sm">
      <CircleAlertIcon className="mt-0.5 size-4 shrink-0" aria-hidden />
      <div>{children}</div>
    </div>
  );
}
