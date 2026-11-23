import { useQuery } from "@tanstack/react-query";
import { api } from "@mr-krabs/api-client";

import { applyHouseholdLanguage } from "@/i18n/language";

export const bootstrapKey = ["app", "bootstrap"] as const;

/** What the app needs at start; `household` is null until onboarding (BootstrapDto). */
export function useBootstrap() {
  return useQuery({
    queryKey: bootstrapKey,
    queryFn: async () => {
      const boot = await api.app.bootstrap();
      // Before the data renders, so the shell's first paint is already in that language.
      if (boot.household) applyHouseholdLanguage(boot.settings?.locale ?? boot.household.locale);
      return boot;
    },
  });
}
