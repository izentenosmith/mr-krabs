import { useTranslation } from "react-i18next";

import { ErrorAlert } from "@/components/ErrorAlert";
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
    <ErrorAlert>
      <p>{t(errorCopyKey(code))}</p>
    </ErrorAlert>
  );
}
