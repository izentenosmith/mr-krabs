import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import type { AccountDto, NewTxInput, TxDetailDto } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

import type { CategoryOptions } from "./TransactionsPage";

type TxType = NewTxInput["type"];
const TYPES: TxType[] = ["expense", "income"];

interface FormValues {
  type: TxType;
  accountId: string;
  categoryId: string;
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

function defaults(): FormValues {
  return {
    type: "expense",
    accountId: "",
    categoryId: "",
    amount: "",
    date: today(),
    description: "",
    memo: "",
  };
}

const optional = (s: string) => (s.trim() === "" ? null : s);

/**
 * Builds `NewTxInput` from what was typed. The amount stays text; its currency is the
 * account's code. Rust parses and validates everything (design Q1, rule 5).
 */
function toInput(v: FormValues, accounts: AccountDto[]): NewTxInput {
  const currency = accounts.find((a) => a.id === v.accountId)?.currency ?? "";
  return {
    type: v.type as "expense" | "income",
    accountId: v.accountId,
    categoryId: v.categoryId,
    amount: { amount: v.amount, currency },
    date: v.date,
    description: optional(v.description),
    memo: optional(v.memo),
  };
}

const control =
  "border-input h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50";

function Field({ id, label, children }: { id: string; label: string; children: React.ReactNode }) {
  return (
    <div className="grid gap-1.5">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      {children}
    </div>
  );
}

/** Add/edit dialog for expense, income and transfer. Presentational: input out, errors in. */
export function TxDialog({ open, onOpenChange, mode, accounts, categories, onSubmit }: TxDialogProps) {
  const { t } = useTranslation();
  const { register, handleSubmit, watch, formState } = useForm<FormValues>({
    defaultValues: defaults(),
  });
  const type = watch("type");
  const accountId = watch("accountId");
  const currency = accounts.find((a) => a.id === accountId)?.currency;
  const categoryOptions = type === "income" ? categories.income : categories.expense;

  const submit = handleSubmit(async (v) => {
    await onSubmit(toInput(v, accounts));
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

          <Field id="tx-account" label={t("tx.form.account")}>
            <select id="tx-account" {...register("accountId")} className={control}>
              <option value="">{t("tx.form.choose")}</option>
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </Field>

          <Field id="tx-category" label={t("tx.form.category")}>
            <select id="tx-category" {...register("categoryId")} className={control}>
              <option value="">{t("tx.form.choose")}</option>
              {categoryOptions.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </Field>

          <div className="grid grid-cols-2 gap-4">
            <Field id="tx-amount" label={t("tx.form.amount")}>
              <div className="relative">
                <input
                  id="tx-amount"
                  inputMode="decimal"
                  autoComplete="off"
                  {...register("amount")}
                  className={`${control} pr-12 text-right tabular-nums`}
                />
                {currency && (
                  <span className="text-muted-foreground pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs">
                    {currency}
                  </span>
                )}
              </div>
            </Field>
            <Field id="tx-date" label={t("tx.form.date")}>
              <input id="tx-date" type="date" {...register("date")} className={control} />
            </Field>
          </div>

          <Field id="tx-description" label={t("tx.form.description")}>
            <input id="tx-description" {...register("description")} className={control} />
          </Field>
          <Field id="tx-memo" label={t("tx.form.memo")}>
            <input id="tx-memo" {...register("memo")} className={control} />
          </Field>

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
