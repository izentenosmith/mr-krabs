import type { LucideIcon } from "lucide-react";

/** Empty / not-yet-available block in place of a view's content (MASTER §7.2). */
export function StateBlock({ icon: Icon, title, body }: { icon: LucideIcon; title: string; body: string }) {
  return (
    <div className="text-muted-foreground flex flex-1 flex-col items-center justify-center gap-2 rounded-lg border p-8 text-center">
      <Icon aria-hidden className="size-6" />
      <p className="text-base font-semibold">{title}</p>
      <p className="text-sm">{body}</p>
    </div>
  );
}
