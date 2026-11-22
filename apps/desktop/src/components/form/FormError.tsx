import { CircleAlertIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { NotAvailable } from "@/components/NotAvailable";
import { errorCopyKey } from "@/lib/api-errors";

/**
 * Form-level server error, from `errors.root.server.type` (the error code). NOT_IMPLEMENTED
 * is the not-yet-available message, not an error (MASTER §7.2).
 */
export function FormError({ code }: { code?: string }) {
  const { t } = useTranslation();
  if (!code) return null;
  if (code === "NOT_IMPLEMENTED") return <NotAvailable />;
  return (
    <div role="alert" className="text-destructive flex items-start gap-2 text-sm">
      <CircleAlertIcon className="mt-0.5 size-4 shrink-0" aria-hidden />
      <p>{t(errorCopyKey(code))}</p>
    </div>
  );
}
