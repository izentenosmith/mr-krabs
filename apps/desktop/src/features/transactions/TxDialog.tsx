import { useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useTranslation } from "react-i18next";
import type { AccountDto, NewTxInput, TxDetailDto } from "@mr-krabs/api-client";

import { Field, controlClass } from "@/components/form/Field";
import { FormError } from "@/components/form/FormError";
import { MoneyInput } from "@/components/form/MoneyInput";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { applyServerErrors, fieldMessage } from "@/lib/api-errors";

const TYPES = ["expense", "income", "transfer"] as const satisfies readonly NewTxInput["type"][];

/** The dialog's form fields (a transfer's source account is `accountId`). */
type TxField = "accountId" | "categoryId" | "toAccountId" | "amount" | "date" | "description" | "memo";

/**
 * `ApiError.fields` wire paths of `NewTxInput` (X-8) → the form field that shows them, for
 * the fields visible for `type`. A path for a hidden field reaches the form-level error.
 */
function fieldMapFor(type: (typeof TYPES)[number]): Partial<Record<string, TxField>> {
  const common = {
    amount: "amount",
    "amount.amount": "amount",
    "amount.currency": "amount",
    date: "date",
    description: "description",
    memo: "memo",
  } as const;
  return type === "transfer"
    ? { ...common, fromAccountId: "accountId", toAccountId: "toAccountId" }
    : { ...common, accountId: "accountId", categoryId: "categoryId" };
}

const REQUIRED = "errors.field.required";
const present = (s: string) => s.trim() !== "";

// Shape only: which fields must be filled for each type. Whether an amount, date or pair
// of accounts is valid is Rust's call (CLAUDE.md rule 5).
const schema = z
  .object({
    type: z.enum(TYPES),
    accountId: z.string(),
    categoryId: z.string(),
    /** Transfer only; for a transfer `accountId` is the source account. */
    toAccountId: z.string(),
    amount: z.string(),
    date: z.string(),
    description: z.string(),
    memo: z.string(),
  })
  .superRefine((v, ctx) => {
    const required: TxField[] = ["accountId", "amount", "date"];
    required.push(v.type === "transfer" ? "toAccountId" : "categoryId");
    for (const field of required) {
      if (!present(v[field])) ctx.addIssue({ code: "custom", path: [field], message: REQUIRED });
    }
  });
type FormValues = z.infer<typeof schema>;

export interface CategoryOption {
  id: string;
  name: string;
  /** The root it belongs to, shown as an `<optgroup>`; none for a childless root. */
  group?: string;
}

/** Category choices per transaction type, from `category_tree` (REQ-13). */
export interface CategoryOptions {
  expense: CategoryOption[];
  income: CategoryOption[];
}

export interface TxDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  mode: "create" | "edit";
  initial?: TxDetailDto;
  accounts: AccountDto[];
  categories: CategoryOptions;
  onSubmit: (input: NewTxInput) => Promise<void>;
  /** Edit mode only: deletes the transaction (undo is offered by the caller). */
  onDelete?: () => Promise<void>;
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

const optional = (s: string) => (present(s) ? s : null);

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

/** Options in order, consecutive ones of the same root inside one `<optgroup>`. */
function CategoryOptionList({ options }: { options: CategoryOption[] }) {
  const runs: Array<{ group?: string; items: CategoryOption[] }> = [];
  for (const o of options) {
    const last = runs.at(-1);
    if (last && o.group && last.group === o.group) last.items.push(o);
    else runs.push({ group: o.group, items: [o] });
  }
  const opts = (items: CategoryOption[]) =>
    items.map((c) => (
      <option key={c.id} value={c.id}>
        {c.name}
      </option>
    ));
  return runs.map((run, i) =>
    run.group ? (
      <optgroup key={`${run.group}-${i}`} label={run.group}>
        {opts(run.items)}
      </optgroup>
    ) : (
      opts(run.items)
    ),
  );
}

/** `<option>`s for the household's accounts. */
export function AccountOptions({ accounts }: { accounts: AccountDto[] }) {
  return accounts.map((a) => (
    <option key={a.id} value={a.id}>
      {a.name}
    </option>
  ));
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
  onDelete,
}: TxDialogProps) {
  const { t } = useTranslation();
  const { register, handleSubmit, watch, setError, setValue, getValues, formState } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: defaults(initial),
  });
  // Messages are i18n keys (ours or the server's); `Field` wires id and aria (MASTER §7.1).
  const err = (field: TxField) => fieldMessage(t, formState.errors[field]?.message);
  const type = watch("type");
  const accountId = watch("accountId");
  const currency = accounts.find((a) => a.id === accountId)?.currency;
  const optionsFor = (kind: FormValues["type"]) => (kind === "income" ? categories.income : categories.expense);
  const categoryOptions = optionsFor(type);
  // A category belongs to one type's list: switching types drops one the new list doesn't
  // have, so what is sent is always what the select shows (F15).
  const typeField = register("type", {
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => {
      const next = e.target.value as FormValues["type"];
      // A transfer has no category; keep the value so switching back restores it (F41).
      if (next === "transfer") return;
      if (!optionsFor(next).some((c) => c.id === getValues("categoryId"))) setValue("categoryId", "");
    },
  });
  const [deleting, setDeleting] = useState(false);
  const busy = deleting || formState.isSubmitting;
  const remove = async () => {
    if (!onDelete || busy) return;
    setDeleting(true);
    try {
      await onDelete();
    } finally {
      setDeleting(false);
    }
  };

  const submit = handleSubmit(async (v) => {
    try {
      await onSubmit(toInput(v, accounts));
    } catch (raw) {
      applyServerErrors(raw, setError, fieldMapFor(v.type));
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
                className="has-[:checked]:bg-accent has-[:focus-visible]:ring-ring/50 flex-1 cursor-pointer rounded px-3 py-1 text-center text-sm has-[:checked]:font-medium has-[:focus-visible]:ring-[3px]"
              >
                <input type="radio" value={value} {...typeField} className="sr-only" />
                {t(`tx.type.${value}`)}
              </label>
            ))}
          </fieldset>

          <Field label={t(type === "transfer" ? "tx.form.from" : "tx.form.account")} error={err("accountId")}>
            <select {...register("accountId")} className={controlClass}>
              <option value="">{t("tx.form.choose")}</option>
              <AccountOptions accounts={accounts} />
            </select>
          </Field>

          {type === "transfer" ? (
            <Field key="to" label={t("tx.form.to")} error={err("toAccountId")}>
              <select {...register("toAccountId")} className={controlClass}>
                <option value="">{t("tx.form.choose")}</option>
                <AccountOptions accounts={accounts} />
              </select>
            </Field>
          ) : (
            <Field key="category" label={t("tx.form.category")} error={err("categoryId")}>
              <select {...register("categoryId")} className={controlClass}>
                <option value="">{t("tx.form.choose")}</option>
                <CategoryOptionList options={categoryOptions} />
              </select>
            </Field>
          )}

          <div className="grid grid-cols-2 gap-4">
            <Field label={t("tx.form.amount")} error={err("amount")}>
              <MoneyInput currency={currency ?? ""} {...register("amount")} />
            </Field>
            <Field label={t("tx.form.date")} error={err("date")}>
              <input type="date" {...register("date")} className={controlClass} />
            </Field>
          </div>

          <Field label={t("tx.form.description")} error={err("description")}>
            <input {...register("description")} className={controlClass} />
          </Field>
          <Field label={t("tx.form.memo")} error={err("memo")}>
            <input {...register("memo")} className={controlClass} />
          </Field>

          <FormError code={formState.errors.root?.server?.type as string | undefined} />

          <DialogFooter>
            {onDelete && (
              <Button
                type="button"
                variant="ghost"
                className="text-destructive sm:mr-auto"
                disabled={busy}
                onClick={() => void remove()}
              >
                {t("tx.form.delete")}
              </Button>
            )}
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t("tx.form.cancel")}
            </Button>
            <Button type="submit" disabled={busy}>
              {t("tx.form.save")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
