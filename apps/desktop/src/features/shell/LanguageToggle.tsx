import { useTranslation } from "react-i18next";
import type { Locale } from "@mr-krabs/api-client";

import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { chooseLanguage } from "@/i18n/language";
import { cn } from "@/lib/utils";

// Item labels and accessible names stay in their own language (MASTER §5.3).
const ITEMS: { value: Locale; label: string; name: string }[] = [
  { value: "es-CL", label: "ES", name: "Español" },
  { value: "en", label: "EN", name: "English" },
];

/** Per-device ES | EN switch (DEC-16, MASTER §5.3): instant, no reload, no toast. */
export function LanguageToggle({ className }: { className?: string }) {
  const { t, i18n } = useTranslation();
  return (
    <ToggleGroup
      type="single"
      size="sm"
      aria-label={t("language.label")}
      value={i18n.resolvedLanguage}
      // Clicking the active item reports "": a language is always selected.
      onValueChange={(v) => {
        const item = ITEMS.find((i) => i.value === v);
        if (item) chooseLanguage(item.value);
      }}
      className={cn("text-xs", className)}
    >
      {ITEMS.map((item) => (
        <ToggleGroupItem
          key={item.value}
          value={item.value}
          aria-label={item.name}
          className="text-muted-foreground data-[state=on]:text-foreground h-6 min-w-8 px-1.5 text-xs"
        >
          {item.label}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  );
}
