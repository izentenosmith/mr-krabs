import { useForm, type FieldErrors } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { CircleAlert, Hourglass } from "lucide-react";
import { ApiError, type AccountDto, type NewTxInput, type TxDetailDto } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

import type { CategoryOptions } from "./TransactionsPage";
import { errorCopyKey, serverFieldErrors, type TxField } from "./txErrors";

type TxType = NewTxInput["type"];
const TYPES: TxType[] = ["expense", "income", "transfer"];

interface FormValues {
  type: TxType;
  accountId: string;
  categoryId: string;
  /** Transfer only; for a transfer `accountId` is the source account. */
  toAccountId: string;
  amount: string;
  date: string;
  description: string;
  memo: string;
}

export interface TxDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mode: "create" | "edit";
  initial?: TxDetailDto;
  accounts: AccountDto[];
  categories: CategoryOptions;
  onSubmit: (input: NewTxInput) => Promise<void>;
}

/** Today's civil date (`YYYY-MM-DD`) in local time, the dialog's default date. */
function today() {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function defaults(initial?: TxDetailDto): FormValues {
  if (!initial || initial.kind === "opening_balance") {
    return {
      type: "expense",
      accountId: "",
      categoryId: "",
      toAccountId: "",
      amount: "",
      date: today(),
      description: "",
      memo: "",
    };
  }
  return {
    type: initial.kind,
    accountId: initial.accountId,
    categoryId: initial.categoryId ?? "",
    toAccountId: initial.toAccountId ?? "",
    // The detail amount is positive and in Rust's es-CL text, which Rust parses back
    // (never the row's signed amount, X-7).
    amount: initial.amount.display,
    date: initial.date,
    description: initial.description,
    memo: initial.memo,
  };
}

const optional = (s: string) => (s.trim() === "" ? null : s);

/**
 * Builds `NewTxInput` from what was typed. The amount stays text; its currency is the
 * account's code. Rust parses and validates everything (design Q1, rule 5).
 */
function toInput(v: FormValues, accounts: AccountDto[]): NewTxInput {
  const currency = accounts.find((a) => a.id === v.accountId)?.currency ?? "";
  const amount = { amount: v.amount, currency };
  const text = { date: v.date, description: optional(v.description), memo: optional(v.memo) };
  if (v.type === "transfer") {
    return { type: "transfer", fromAccountId: v.accountId, toAccountId: v.toAccountId, amount, ...text };
  }
  return { type: v.type, accountId: v.accountId, categoryId: v.categoryId, amount, ...text };
}

const control =
  "border-input h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50";

function Field({
  id,
  label,
  error,
  children,
}: {
  id: string;
  label: string;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="grid gap-1.5">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      {children}
      {error && (
        <p id={`${id}-error`} className="text-destructive flex items-center gap-1 text-xs">
          <CircleAlert aria-hidden className="size-3.5" />
          {error}
        </p>
      )}
    </div>
  );
}

/** The form-level line above the footer (MASTER §7.1, §7.2). */
function FormMessage({ text, notAvailable }: { text?: string; notAvailable: boolean }) {
  if (!text) return null;
  if (notAvailable) {
    return (
      <p role="status" className="text-muted-foreground flex items-center gap-2 text-sm">
        <Hourglass aria-hidden className="size-4" />
        {text}
      </p>
    );
  }
  return (
    <p role="alert" className="text-destructive flex items-center gap-2 text-sm">
      <CircleAlert aria-hidden className="size-4" />
      {text}
    </p>
  );
}

/** `aria-*` that ties a control to its error line (MASTER §7.1). */
function invalid(id: string, error: string | undefined) {
  return error ? { "aria-invalid": true, "aria-describedby": `${id}-error` } : {};
}

/** Add/edit dialog for expense, income and transfer. Presentational: input out, errors in. */
export function TxDialog({
  open,
  onOpenChange,
  mode,
  initial,
  accounts,
  categories,
  onSubmit,
}: TxDialogProps) {
  const { t, i18n } = useTranslation();
  const { register, handleSubmit, watch, setError, formState } = useForm<FormValues>({
    defaultValues: defaults(initial),
  });
  // Messages are i18n keys (ours or the server's); an unknown key gets the generic text.
  const message = (errors: FieldErrors<FormValues>, field: TxField) => {
    const key = errors[field]?.message;
    if (!key) return undefined;
    return i18n.exists(key) ? t(key as never) : t("errors.field.invalid");
  };
  const err = (field: TxField) => message(formState.errors, field);
  const type = watch("type");
  const accountId = watch("accountId");
  const currency = accounts.find((a) => a.id === accountId)?.currency;
  const categoryOptions = type === "income" ? categories.income : categories.expense;

  const submit = handleSubmit(async (v) => {
    try {
      await onSubmit(toInput(v, accounts));
    } catch (raw) {
      const error = ApiError.from(raw);
      const fields = serverFieldErrors(error);
      for (const [field, key] of fields) setError(field, { message: key });
      if (fields.length === 0) setError("root", { type: error.code, message: errorCopyKey(error.code) });
    }
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={false} aria-describedby={undefined}>
        <DialogHeader>
          <DialogTitle>{t(mode === "create" ? "tx.form.newTitle" : "tx.form.editTitle")}</DialogTitle>
        </DialogHeader>
        <form onSubmit={submit} noValidate className="grid gap-4">
          <fieldset className="flex gap-1 rounded-md border p-1">
            <legend className="sr-only">{t("tx.type.legend")}</legend>
            {TYPES.map((value) => (
              <label
                key={value}
                className="has-[:checked]:bg-accent flex-1 cursor-pointer rounded px-3 py-1 text-center text-sm has-[:checked]:font-medium"
              >
                <input type="radio" value={value} {...register("type")} className="sr-only" />
                {t(`tx.type.${value}`)}
              </label>
            ))}
          </fieldset>

          <Field
            id="tx-account"
            label={t(type === "transfer" ? "tx.form.from" : "tx.form.account")}
            error={err("accountId")}
          >
            <select
              id="tx-account"
              {...register("accountId")}
              {...invalid("tx-account", err("accountId"))}
              className={control}
            >
              <option value="">{t("tx.form.choose")}</option>
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </Field>

          {type === "transfer" ? (
            <Field id="tx-to" label={t("tx.form.to")} error={err("toAccountId")}>
              <select
                id="tx-to"
                {...register("toAccountId")}
                {...invalid("tx-to", err("toAccountId"))}
                className={control}
              >
                <option value="">{t("tx.form.choose")}</option>
                {accounts.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.name}
                  </option>
                ))}
              </select>
            </Field>
          ) : (
            <Field id="tx-category" label={t("tx.form.category")} error={err("categoryId")}>
              <select
                id="tx-category"
                {...register("categoryId")}
                {...invalid("tx-category", err("categoryId"))}
                className={control}
              >
                <option value="">{t("tx.form.choose")}</option>
                {categoryOptions.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </Field>
          )}

          <div className="grid grid-cols-2 gap-4">
            <Field id="tx-amount" label={t("tx.form.amount")} error={err("amount")}>
              <div className="relative">
                <input
                  id="tx-amount"
                  inputMode="decimal"
                  autoComplete="off"
                  {...register("amount")}
                  {...invalid("tx-amount", err("amount"))}
                  className={`${control} pr-12 text-right tabular-nums`}
                />
                {currency && (
                  <span className="text-muted-foreground pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs">
                    {currency}
                  </span>
                )}
              </div>
            </Field>
            <Field id="tx-date" label={t("tx.form.date")} error={err("date")}>
              <input
                id="tx-date"
                type="date"
                {...register("date")}
                {...invalid("tx-date", err("date"))}
                className={control}
              />
            </Field>
          </div>

          <Field id="tx-description" label={t("tx.form.description")} error={err("description")}>
            <input
              id="tx-description"
              {...register("description")}
              {...invalid("tx-description", err("description"))}
              className={control}
            />
          </Field>
          <Field id="tx-memo" label={t("tx.form.memo")} error={err("memo")}>
            <input
              id="tx-memo"
              {...register("memo")}
              {...invalid("tx-memo", err("memo"))}
              className={control}
            />
          </Field>

          <FormMessage
            notAvailable={formState.errors.root?.type === "NOT_IMPLEMENTED"}
            text={formState.errors.root?.message && t(formState.errors.root.message as never)}
          />

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t("tx.form.cancel")}
            </Button>
            <Button type="submit" disabled={formState.isSubmitting}>
              {t("tx.form.save")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
