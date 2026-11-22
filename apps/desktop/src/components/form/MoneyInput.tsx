import type { ComponentProps } from "react";

import { cn } from "@/lib/utils";

import { controlClass } from "./Field";

/**
 * Amount as text, sent to Rust as typed (CLAUDE.md rule 5): no parsing or formatting here.
 * The currency code is a muted suffix (MASTER §7.1).
 */
export function MoneyInput({
  currency,
  className,
  ...props
}: ComponentProps<"input"> & { currency: string }) {
  return (
    <div className="relative">
      <input
        type="text"
        inputMode="decimal"
        autoComplete="off"
        className={cn(controlClass, "pr-14 text-right tabular-nums", className)}
        {...props}
      />
      <span
        aria-hidden
        className="text-muted-foreground pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs"
      >
        {currency}
      </span>
    </div>
  );
}
