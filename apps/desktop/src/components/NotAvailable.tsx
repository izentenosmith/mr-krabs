import { HourglassIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

/**
 * A command answered NOT_IMPLEMENTED (design Q12). Nothing is wrong, so it is never styled
 * as an error (MASTER §7.2).
 */
export function NotAvailable({ className }: { className?: string }) {
  const { t } = useTranslation();
  return (
    <div role="status" className={cn("text-muted-foreground flex items-start gap-2 text-sm", className)}>
      <HourglassIcon className="mt-0.5 size-4 shrink-0" aria-hidden />
      <div>
        <p className="font-medium">{t("states.notAvailableTitle")}</p>
        <p className="text-xs">{t("states.notAvailableBody")}</p>
      </div>
    </div>
  );
}
