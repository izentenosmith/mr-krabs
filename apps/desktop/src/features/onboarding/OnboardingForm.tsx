import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import type { HouseholdInput } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";
import { controlClass, Field } from "@/components/form/Field";
import { FormError } from "@/components/form/FormError";
import { applyServerErrors, fieldMessage } from "@/lib/api-errors";
import { CURRENCIES } from "@/lib/currencies";

const required = (s: string) => s.trim().length > 0;

// Shape only; the core validates the values (CLAUDE.md rule 5).
const schema = z.object({
  name: z.string().refine(required, "errors.field.required"),
  myName: z.string().refine(required, "errors.field.required"),
  baseCurrency: z.enum(CURRENCIES),
});
type FormValues = z.infer<typeof schema>;

export interface OnboardingFormProps {
  /** Rejects with the core's error; the form shows it. */
  onSubmit: (input: HouseholdInput) => Promise<unknown>;
}

/** First launch: creates the household (MASTER §5.2). Presentational. */
export function OnboardingForm({ onSubmit }: OnboardingFormProps) {
  const { t } = useTranslation();
  const {
    register,
    handleSubmit,
    setError,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: "", myName: "", baseCurrency: "CLP" },
  });

  const submit = handleSubmit(async (values) => {
    try {
      await onSubmit({
        name: values.name.trim(),
        myName: values.myName.trim(),
        baseCurrency: values.baseCurrency,
      });
    } catch (err) {
      applyServerErrors(err, setError, { name: "name", myName: "myName", baseCurrency: "baseCurrency" });
    }
  });

  return (
    <div className="bg-background flex min-h-screen items-center justify-center p-8">
      <div className="bg-card text-card-foreground w-full max-w-md rounded-lg border p-6">
        <img src="/favicon.png" alt="" className="mb-4 size-10" />
        <h1 className="text-xl font-semibold">{t("onboarding.title")}</h1>
        <p className="text-muted-foreground mt-1 text-sm">{t("onboarding.description")}</p>
        <form onSubmit={submit} noValidate className="mt-6 flex flex-col gap-4">
          <Field label={t("onboarding.householdName")} error={fieldMessage(t, errors.name?.message)}>
            <input className={controlClass} autoComplete="off" {...register("name")} />
          </Field>
          <Field label={t("onboarding.myName")} error={fieldMessage(t, errors.myName?.message)}>
            <input className={controlClass} autoComplete="name" {...register("myName")} />
          </Field>
          <Field
            label={t("onboarding.baseCurrency")}
            error={fieldMessage(t, errors.baseCurrency?.message)}
          >
            <select className={controlClass} {...register("baseCurrency")}>
              {CURRENCIES.map((c) => (
                <option key={c} value={c}>
                  {t(`currency.${c}`)}
                </option>
              ))}
            </select>
          </Field>
          <FormError code={errors.root?.server?.type?.toString()} />
          <Button type="submit" className="w-full" disabled={isSubmitting}>
            {t("onboarding.submit")}
          </Button>
        </form>
      </div>
    </div>
  );
}
