import { useQuery } from "@tanstack/react-query";
import { api } from "@mr-krabs/api-client";

export const bootstrapKey = ["app", "bootstrap"] as const;

/** What the app needs at start; `household` is null until onboarding (BootstrapDto). */
export function useBootstrap() {
  return useQuery({ queryKey: bootstrapKey, queryFn: api.app.bootstrap });
}
