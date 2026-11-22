import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import type { AccountDetailDto, AccountInput, AccountKind } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { controlClass, Field } from "@/components/form/Field";
import { FormError } from "@/components/form/FormError";
import { MoneyInput } from "@/components/form/MoneyInput";
import { NotAvailable } from "@/components/NotAvailable";
import { applyServerErrors, fieldMessage } from "@/lib/api-errors";
import { CURRENCIES } from "@/lib/currencies";

import { ACCOUNT_KINDS } from "./groups";

const filled = (s: string) => s.trim().length > 0;

// Shape only: whether the amount is valid is decided by Rust (`parse_es_cl`).
const schema = z
  .object({
    name: z.string().refine(filled, "errors.field.required"),
    kind: z.enum(ACCOUNT_KINDS as [AccountKind, ...AccountKind[]]),
    currency: z.string(),
    institution: z.string(),
    numberMask: z.string(),
    openingAmount: z.string(),
    openingDate: z.string(),
  })
  .refine((v) => !filled(v.openingAmount) || filled(v.openingDate), {
    path: ["openingDate"],
    message: "errors.field.required",
  });
type FormValues = z.infer<typeof schema>;

/** Wire path in `ApiError.fields` → form field (X-8). */
const FIELD_MAP = {
  name: "name",
  kind: "kind",
  currency: "currency",
  institution: "institution",
  numberMask: "numberMask",
  "openingBalance.amount": "openingAmount",
  "openingBalance.date": "openingDate",
} as const;

export type AccountDialogState =
  | { mode: "create"; baseCurrency: string }
  | { mode: "edit"; status: "loading" | "notAvailable" }
  | { mode: "edit"; status: "error"; code: string }
  | { mode: "edit"; status: "ready"; detail: AccountDetailDto };

export interface AccountDialogProps {
  state: AccountDialogState;
  onOpenChange: (open: boolean) => void;
  /** Rejects with the core's error; the form shows it. */
  onSubmit: (input: AccountInput) => Promise<unknown>;
  onArchive: () => Promise<unknown>;
}

/** Create or edit an account. Presentational: takes DTOs and callbacks. */
export function AccountDialog({ state, onOpenChange, onSubmit, onArchive }: AccountDialogProps) {
  const { t } = useTranslation();
  const edit = state.mode === "edit";
  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{edit ? t("accounts.editTitle") : t("accounts.new")}</DialogTitle>
          <DialogDescription>{t("accounts.dialogDescription")}</DialogDescription>
        </DialogHeader>
        {state.mode === "create" && (
          <AccountForm
            defaults={emptyValues(state.baseCurrency)}
            onSubmit={onSubmit}
            onCancel={() => onOpenChange(false)}
          />
        )}
        {state.mode === "edit" && state.status === "ready" && (
          <AccountForm
            detail={state.detail}
            defaults={valuesFrom(state.detail)}
            onSubmit={onSubmit}
            onArchive={onArchive}
            onCancel={() => onOpenChange(false)}
          />
        )}
        {state.mode === "edit" && state.status === "loading" && (
          <p aria-busy className="text-muted-foreground text-sm">
            {t("shell.loading")}
          </p>
        )}
        {state.mode === "edit" && state.status === "notAvailable" && <NotAvailable />}
        {state.mode === "edit" && state.status === "error" && <FormError code={state.code} />}
      </DialogContent>
    </Dialog>
  );
}

function today() {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function emptyValues(currency: string): FormValues {
  return {
    name: "",
    kind: "checking",
    currency,
    institution: "",
    numberMask: "",
    openingAmount: "",
    openingDate: today(),
  };
}

function valuesFrom({ account, openingBalance }: AccountDetailDto): FormValues {
  return {
    name: account.name,
    kind: account.kind,
    currency: account.currency,
    institution: account.institution ?? "",
    numberMask: account.numberMask ?? "",
    // The core's display text parses back to the same amount (parse_es_cl strips "$").
    openingAmount: openingBalance?.amount.display ?? "",
    openingDate: openingBalance?.date ?? today(),
  };
}

const orNull = (s: string) => (filled(s) ? s.trim() : null);

function toInput(v: FormValues, detail?: AccountDetailDto): AccountInput {
  return {
    name: v.name.trim(),
    kind: v.kind,
    currency: v.currency,
    // account_update replaces the whole account (X-4 D5): keep what the form doesn't edit.
    ...(detail && { scope: detail.account.scope, visibility: detail.account.visibility }),
    institution: orNull(v.institution),
    numberMask: orNull(v.numberMask),
    openingBalance: filled(v.openingAmount)
      ? { amount: { amount: v.openingAmount, currency: v.currency }, date: v.openingDate }
      : null,
  };
}

function AccountForm({
  detail,
  defaults,
  onSubmit,
  onArchive,
  onCancel,
}: {
  detail?: AccountDetailDto;
  defaults: FormValues;
  onSubmit: (input: AccountInput) => Promise<unknown>;
  onArchive?: () => Promise<unknown>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const {
    register,
    handleSubmit,
    setError,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema), defaultValues: defaults });

  const submit = handleSubmit(async (values) => {
    try {
      await onSubmit(toInput(values, detail));
    } catch (err) {
      applyServerErrors(err, setError, FIELD_MAP);
    }
  });

  const archive = async () => {
    try {
      await onArchive?.();
    } catch (err) {
      applyServerErrors(err, setError, {});
    }
  };

  const msg = (key?: string) => fieldMessage(t, key);

  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-4">
      <Field label={t("accounts.name")} error={msg(errors.name?.message)}>
        <input className={controlClass} autoComplete="off" {...register("name")} />
      </Field>
      <div className="grid grid-cols-2 gap-4">
        <Field label={t("accounts.kindLabel")} error={msg(errors.kind?.message)}>
          <select className={controlClass} {...register("kind")}>
            {ACCOUNT_KINDS.map((k) => (
              <option key={k} value={k}>
                {t(`accounts.kind.${k}`)}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t("accounts.currencyLabel")} error={msg(errors.currency?.message)}>
          <select className={controlClass} {...register("currency")}>
            {CURRENCIES.map((c) => (
              <option key={c} value={c}>
                {t(`currency.${c}`)}
              </option>
            ))}
          </select>
        </Field>
      </div>
      <div className="grid grid-cols-2 gap-4">
        <Field label={t("accounts.institution")} error={msg(errors.institution?.message)}>
          <input className={controlClass} autoComplete="off" {...register("institution")} />
        </Field>
        <Field label={t("accounts.numberMask")} error={msg(errors.numberMask?.message)}>
          <input className={controlClass} autoComplete="off" {...register("numberMask")} />
        </Field>
      </div>
      <div className="grid grid-cols-2 gap-4">
        <Field label={t("accounts.openingAmount")} error={msg(errors.openingAmount?.message)}>
          <MoneyInput currency={watch("currency")} {...register("openingAmount")} />
        </Field>
        <Field label={t("accounts.openingDate")} error={msg(errors.openingDate?.message)}>
          <input type="date" className={controlClass} {...register("openingDate")} />
        </Field>
      </div>
      <FormError code={errors.root?.server?.type?.toString()} />
      <DialogFooter className="sm:justify-between">
        <div>
          {onArchive && (
            <Button type="button" variant="ghost" className="text-destructive" onClick={archive} disabled={isSubmitting}>
              {t("accounts.archive")}
            </Button>
          )}
        </div>
        <div className="flex gap-2">
          <Button type="button" variant="outline" onClick={onCancel}>
            {t("accounts.cancel")}
          </Button>
          <Button type="submit" disabled={isSubmitting}>
            {t("accounts.save")}
          </Button>
        </div>
      </DialogFooter>
    </form>
  );
}
