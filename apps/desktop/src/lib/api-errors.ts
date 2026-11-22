import type { TFunction } from "i18next";
import { ApiError, type ErrorCode } from "@mr-krabs/api-client";
import type { FieldValues, Path, UseFormSetError } from "react-hook-form";

import i18n from "@/i18n";

/** Form-level server error, kept under react-hook-form's `root.server` with the code as type. */
export const SERVER_ERROR = "root.server" as const;

export function isNotImplemented(error: unknown): boolean {
  return ApiError.from(error).code === "NOT_IMPLEMENTED";
}

/** i18n key of the user-facing copy for an error code (MASTER §9: never the raw code). */
export function errorCopyKey(code: ErrorCode | string) {
  const key = `errors.code.${code}`;
  return (i18n.exists(key) ? key : "errors.code.INTERNAL") as "errors.code.INTERNAL";
}

/**
 * Text for a field error. Messages are i18n keys — set by zod (`errors.field.required`) or
 * sent by the core in `ApiError.fields` (X-8); a key the UI doesn't know gets the generic
 * "check this field" copy.
 */
export function fieldMessage(t: TFunction, key: string | undefined) {
  if (!key) return undefined;
  return hasFieldCopy(key) ? t(key as "errors.field.unknown") : t("errors.field.unknown");
}

/** Only `errors.field.*` keys with copy: a value naming any other key reads as generic. */
function hasFieldCopy(key: string) {
  return key.startsWith("errors.field.") && i18n.exists(key);
}

/**
 * Puts a rejected command's error on the form: `fields` (wire path → i18n key) under the
 * matching form fields; the form-level error (the code as its type) when a field can't be
 * shown or none was named, and for a code other than VALIDATION whose field has no copy.
 * `fieldMap` maps wire paths (`openingBalance.amount`) to form field names.
 */
export function applyServerErrors<T extends FieldValues>(
  raw: unknown,
  setError: UseFormSetError<T>,
  fieldMap: Partial<Record<string, Path<T>>>,
) {
  const error = ApiError.from(raw);
  let unmapped = false;
  let generic = false;
  for (const [path, key] of Object.entries(error.fields ?? {})) {
    const field = fieldMap[path];
    if (field && key) {
      setError(field, { type: "server", message: key });
      if (!hasFieldCopy(key)) generic = true;
    } else unmapped = true;
  }
  // Form-level copy whenever a field can't say what happened itself: an unmapped path, no
  // fields, or (for a code other than VALIDATION) a field whose key has no copy of its own.
  // A field with its own `errors.field.*` copy is enough, so a locked currency is not also
  // called a mismatch and a mismatch isn't said twice (X-13, Q-ui-transactions-5).
  const fieldless = !error.fields || Object.keys(error.fields).length === 0;
  if (unmapped || fieldless || (error.code !== "VALIDATION" && generic)) {
    setError(SERVER_ERROR, { type: error.code });
  }
}

/**
 * Query retry policy: only a transport failure (IPC/HTTP) may be transient. Any answer from
 * the core — NOT_IMPLEMENTED included — is final, so it shows at once instead of after
 * TanStack Query's default three back-off retries.
 */
export function shouldRetry(failureCount: number, error: unknown): boolean {
  return failureCount < 3 && ApiError.from(error).code === "TRANSPORT";
}

/** How a failed load shows: not yet available (NOT_IMPLEMENTED) or an error with its code. */
export function failureState(error: unknown): { status: "notAvailable" } | { status: "error"; code: string } {
  const { code } = ApiError.from(error);
  return code === "NOT_IMPLEMENTED" ? { status: "notAvailable" } : { status: "error", code };
}
