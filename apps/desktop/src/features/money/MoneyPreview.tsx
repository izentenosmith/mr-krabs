import { useState, type FormEvent } from "react";
import type { MoneyDto } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export interface MoneyPreviewProps {
  onParse: (amount: string) => Promise<MoneyDto>;
}

/**
 * Sends user text to Rust and shows what Rust returns. The UI never parses or formats
 * money itself (docs/04-command-boundary.md).
 */
export function MoneyPreview({ onParse }: MoneyPreviewProps) {
  const [amount, setAmount] = useState("1.234.567");
  const [result, setResult] = useState<MoneyDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      setResult(await onParse(amount));
    } catch (err) {
      setResult(null);
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Montos en CLP</CardTitle>
        <CardDescription>Rust interpreta y formatea; React solo muestra.</CardDescription>
      </CardHeader>
      <CardContent>
        <form onSubmit={submit} className="flex gap-2">
          <label className="sr-only" htmlFor="amount">
            Monto
          </label>
          <input
            id="amount"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            className="border-input h-9 flex-1 rounded-md border bg-transparent px-3 text-sm"
          />
          <Button type="submit">Interpretar</Button>
        </form>
        {result && (
          <p className="mt-3 text-sm" data-testid="money-result">
            <span className="text-2xl font-semibold">{result.display}</span>{" "}
            <span className="text-muted-foreground font-mono">
              ({result.minor} {result.currency} en unidades mínimas)
            </span>
          </p>
        )}
        {error && (
          <p role="alert" className="text-destructive mt-3 text-sm">
            {error}
          </p>
        )}
      </CardContent>
    </Card>
  );
}
