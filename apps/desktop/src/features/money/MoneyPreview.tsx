import { useState } from "react";
import { useForm } from "react-hook-form";
import { useTranslation } from "react-i18next";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import type { MoneyDto } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export interface MoneyPreviewProps {
  onParse: (amount: string) => Promise<MoneyDto>;
}

// Shape only. Whether the text is a valid amount is decided by Rust (`money_parse`).
const schema = z.object({ amount: z.string().trim().min(1, "money.required") });
type FormValues = z.infer<typeof schema>;

/**
 * Sends user text to Rust and shows what Rust returns. The UI never parses or formats
 * money itself (docs/04-command-boundary.md).
 */
export function MoneyPreview({ onParse }: MoneyPreviewProps) {
  const { t } = useTranslation();
  const [result, setResult] = useState<MoneyDto | null>(null);
  const {
    register,
    handleSubmit,
    setError,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema), defaultValues: { amount: "1.234.567" } });

  const submit = handleSubmit(async ({ amount }) => {
    try {
      setResult(await onParse(amount));
    } catch (err) {
      setResult(null);
      setError("amount", { message: err instanceof Error ? err.message : String(err) });
    }
  });

  // Client-side messages are i18n keys; backend messages are shown as received.
  const message = errors.amount?.message;
  const errorText = message === "money.required" ? t("money.required") : message;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("money.title")}</CardTitle>
        <CardDescription>{t("money.description")}</CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={submit} className="flex gap-2" noValidate>
          <label className="sr-only" htmlFor="amount">
            {t("money.amount")}
          </label>
          <input
            id="amount"
            aria-invalid={errors.amount ? true : undefined}
            {...register("amount")}
            className="border-input h-9 flex-1 rounded-md border bg-transparent px-3 text-sm"
          />
          <Button type="submit" disabled={isSubmitting}>
            {t("money.parse")}
          </Button>
        </form>
        {result && (
          <p className="mt-3 text-sm" data-testid="money-result">
            <span className="text-2xl font-semibold">{result.display}</span>{" "}
            <span className="text-muted-foreground font-mono">
              {t("money.minorUnits", { minor: result.minor, currency: result.currency })}
            </span>
          </p>
        )}
        {errorText && (
          <p role="alert" className="text-destructive mt-3 text-sm">
            {errorText}
          </p>
        )}
      </CardContent>
    </Card>
  );
}
