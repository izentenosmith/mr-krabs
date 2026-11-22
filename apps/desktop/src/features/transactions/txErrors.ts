import type { ApiError } from "@mr-krabs/api-client";

/** The dialog's form fields that a server error can point at. */
export type TxField =
  | "accountId"
  | "categoryId"
  | "toAccountId"
  | "amount"
  | "date"
  | "description"
  | "memo";

// `ApiError.fields` keys are wire paths of `NewTxInput` (X-8); a dotted path such as
// `amount.amount` belongs to the field named by its first segment.
const FIELD_BY_WIRE: Record<string, TxField> = {
  accountId: "accountId",
  fromAccountId: "accountId",
  toAccountId: "toAccountId",
  categoryId: "categoryId",
  amount: "amount",
  date: "date",
  description: "description",
  memo: "memo",
};

/**
 * Splits server field errors into those a visible form field can show (`[field, i18n key]`)
 * and whether any could not be placed (no form field, or a field hidden for this type).
 */
export function serverFieldErrors(error: ApiError, visible: (field: TxField) => boolean) {
  const placed: Array<[TxField, string]> = [];
  let unplaced = false;
  for (const [path, key] of Object.entries(error.fields ?? {})) {
    const field = FIELD_BY_WIRE[path.split(".")[0] ?? ""];
    if (field && key && visible(field)) placed.push([field, key]);
    else unplaced = true;
  }
  return { placed, unplaced };
}

/** Server field values are `errors.field.<reason>` keys (X-8); anything else is generic. */
export const isFieldErrorKey = (key: string) => key.startsWith("errors.field.");

// Codes with their own copy under `errors.code.*`; anything else reads as INTERNAL.
const COPIED = [
  "VALIDATION",
  "NOT_FOUND",
  "CURRENCY_MISMATCH",
  "UNBALANCED_ENTRY",
  "CONFLICT",
  "INTERNAL",
  "TRANSPORT",
] as const;
type CopiedCode = (typeof COPIED)[number];
const isCopied = (code: string): code is CopiedCode => (COPIED as readonly string[]).includes(code);

/** i18n key for what the user is told about an error code (MASTER §9: never the raw code). */
export function errorCopyKey(code: ApiError["code"]) {
  if (code === "NOT_IMPLEMENTED") return "notAvailable.body" as const;
  return `errors.code.${isCopied(code) ? code : "INTERNAL"}` as const;
}
