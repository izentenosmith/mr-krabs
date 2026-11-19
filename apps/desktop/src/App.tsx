import { useQuery } from "@tanstack/react-query";
import { api, transport } from "@mr-krabs/api-client";

import { HealthCard } from "@/features/health/HealthCard";
import { MoneyPreview } from "@/features/money/MoneyPreview";

export default function App() {
  const health = useQuery({ queryKey: ["app", "health"], queryFn: api.app.health, retry: 1 });

  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col gap-6 p-8">
      <header className="flex items-center gap-3">
        <img src="/favicon.png" alt="" className="size-8" />
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Mr Krabs</h1>
          <p className="text-muted-foreground text-sm">Finanzas personales y del hogar · local-first</p>
        </div>
      </header>
      <HealthCard
        transport={transport}
        loading={health.isPending}
        health={health.data}
        error={health.error?.message}
      />
      <MoneyPreview onParse={(amount) => api.money.parse({ amount, currency: "CLP" })} />
    </main>
  );
}
