import { useId, type ReactElement } from "react";
import { CircleAlertIcon } from "lucide-react";
import { Slot } from "@radix-ui/react-slot";

/** Control classes for inputs and native selects (MASTER §7.1). */
export const controlClass =
  "border-input h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 aria-invalid:border-destructive";

export interface FieldProps {
  label: string;
  /** Already translated message. */
  error?: string;
  /** The control; gets `id`, `aria-invalid` and `aria-describedby`. */
  children: ReactElement;
}

/** Label above the control, error line under it (MASTER §7.1). */
export function Field({ label, error, children }: FieldProps) {
  const id = useId();
  const errorId = `${id}-error`;
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      <Slot id={id} aria-invalid={error ? true : undefined} aria-describedby={error ? errorId : undefined}>
        {children}
      </Slot>
      {error && (
        <p id={errorId} className="text-destructive flex items-center gap-1 text-xs">
          <CircleAlertIcon className="size-3.5 shrink-0" aria-hidden />
          {error}
        </p>
      )}
    </div>
  );
}
