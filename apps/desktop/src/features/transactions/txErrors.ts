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

/** Server field errors as `[form field, i18n key]` pairs; keys with no form field are dropped. */
export function serverFieldErrors(error: ApiError): Array<[TxField, string]> {
  return Object.entries(error.fields ?? {}).flatMap(([path, key]) => {
    const field = FIELD_BY_WIRE[path.split(".")[0] ?? ""];
    return field && key ? [[field, key] as [TxField, string]] : [];
  });
}

type CopiedCode = "VALIDATION" | "NOT_FOUND" | "CURRENCY_MISMATCH" | "UNBALANCED_ENTRY" | "CONFLICT" | "INTERNAL" | "TRANSPORT";
const COPIED = new Set<string>([
  "VALIDATION",
  "NOT_FOUND",
  "CURRENCY_MISMATCH",
  "UNBALANCED_ENTRY",
  "CONFLICT",
  "INTERNAL",
  "TRANSPORT",
]);

/** i18n key for what the user is told about an error code (MASTER §9: never the raw code). */
export function errorCopyKey(code: ApiError["code"]) {
  if (code === "NOT_IMPLEMENTED") return "notAvailable.body" as const;
  return `errors.code.${COPIED.has(code) ? (code as CopiedCode) : "INTERNAL"}` as const;
}
