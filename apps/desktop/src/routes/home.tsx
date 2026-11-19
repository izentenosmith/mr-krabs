import { useQuery } from "@tanstack/react-query";
import { api, transport } from "@mr-krabs/api-client";

import { HealthCard } from "@/features/health/HealthCard";
import { MoneyPreview } from "@/features/money/MoneyPreview";

export function HomePage() {
  const health = useQuery({ queryKey: ["app", "health"], queryFn: api.app.health, retry: 1 });

  return (
    <>
      <HealthCard
        transport={transport}
        loading={health.isPending}
        health={health.data}
        error={health.error?.message}
      />
      <MoneyPreview onParse={(amount) => api.money.parse({ amount, currency: "CLP" })} />
    </>
  );
}
