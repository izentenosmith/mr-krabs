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
  return i18n.exists(key) ? t(key as "errors.field.unknown") : t("errors.field.unknown");
}

/**
 * Puts a rejected command's error on the form: `fields` (wire path → i18n key) under the
 * matching form fields, anything else as the form-level error with the code as its type.
 * `fieldMap` maps wire paths (`openingBalance.amount`) to form field names.
 */
export function applyServerErrors<T extends FieldValues>(
  raw: unknown,
  setError: UseFormSetError<T>,
  fieldMap: Partial<Record<string, Path<T>>>,
) {
  const error = ApiError.from(raw);
  let unmapped = false;
  for (const [path, key] of Object.entries(error.fields ?? {})) {
    const field = fieldMap[path];
    if (field && key) setError(field, { type: "server", message: key });
    else unmapped = true;
  }
  if (!error.fields || unmapped || Object.keys(error.fields).length === 0) {
    setError(SERVER_ERROR, { type: error.code });
  }
}
