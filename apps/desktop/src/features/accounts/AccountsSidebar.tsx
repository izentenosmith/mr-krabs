import { useId } from "react";
import { ArchiveIcon, InboxIcon, PencilIcon, PlusIcon, UserIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AccountSummaryDto } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";
import { ErrorAlert } from "@/components/ErrorAlert";
import { NotAvailable } from "@/components/NotAvailable";
import { errorCopyKey } from "@/lib/api-errors";
import { cn } from "@/lib/utils";

import { groupAccounts, type AccountGroupId } from "./groups";

export type SidebarState =
  | { status: "loading" }
  | { status: "ready"; accounts: AccountSummaryDto[] }
  | { status: "notAvailable" }
  | { status: "error"; code: string };

export interface AccountsSidebarProps {
  state: SidebarState;
  showArchived: boolean;
  onToggleArchived: () => void;
  onCreate: () => void;
  onEdit: (id: string) => void;
  onRetry: () => void;
}

/** Accounts section of the sidebar (MASTER §5.1). Shows `balance.display` as received. */
export function AccountsSidebar({
  state,
  showArchived,
  onToggleArchived,
  onCreate,
  onEdit,
  onRetry,
}: AccountsSidebarProps) {
  const { t } = useTranslation();
  const titleId = useId();

  return (
    <section aria-labelledby={titleId} aria-busy={state.status === "loading" || undefined} className="flex flex-col gap-2">
      <div className="flex items-center justify-between px-2">
        <h2 id={titleId} className="text-muted-foreground text-xs font-medium">
          {t("accounts.title")}
        </h2>
        <Button variant="ghost" size="icon" className="size-7" aria-label={t("accounts.new")} onClick={onCreate}>
          <PlusIcon />
        </Button>
      </div>

      {state.status === "loading" && <p className="text-muted-foreground px-2 text-sm">{t("shell.loading")}</p>}
      {state.status === "notAvailable" && <NotAvailable className="px-2" />}
      {state.status === "error" && (
        <div className="flex flex-col items-start gap-2 px-2">
          <ErrorAlert>
            <p>{t(errorCopyKey(state.code))}</p>
          </ErrorAlert>
          <Button variant="ghost" size="sm" onClick={onRetry}>
            {t("shell.retry")}
          </Button>
        </div>
      )}
      {state.status === "ready" && state.accounts.length === 0 && (
        <div className="text-muted-foreground flex flex-col items-start gap-2 px-2 text-sm">
          <p className="flex items-center gap-2">
            <InboxIcon className="size-4" aria-hidden />
            {t("accounts.empty")}
          </p>
          <Button variant="outline" size="sm" onClick={onCreate}>
            {t("accounts.new")}
          </Button>
        </div>
      )}
      {state.status === "ready" &&
        groupAccounts(state.accounts).map((group) => (
          <AccountGroup key={group.id} id={group.id} accounts={group.accounts} onEdit={onEdit} />
        ))}

      <Button
        variant="ghost"
        size="sm"
        className="text-muted-foreground self-start"
        aria-pressed={showArchived}
        onClick={onToggleArchived}
      >
        {showArchived ? t("accounts.hideArchived") : t("accounts.showArchived")}
      </Button>
    </section>
  );
}

function AccountGroup({
  id,
  accounts,
  onEdit,
}: {
  id: AccountGroupId;
  accounts: AccountSummaryDto[];
  onEdit: (id: string) => void;
}) {
  const { t } = useTranslation();
  const captionId = useId();
  return (
    <div role="group" aria-labelledby={captionId} className="flex flex-col">
      <p id={captionId} className="text-muted-foreground px-2 py-1 text-xs">
        {t(`accounts.group.${id}`)}
      </p>
      <ul>
        {accounts.map(({ account, balance }) => (
          <li key={account.id} className="group hover:bg-accent flex h-8 items-center gap-2 rounded-md px-2">
            <span className={cn("flex min-w-0 flex-1 items-center gap-1.5 text-sm", account.isArchived && "text-muted-foreground")}>
              <span className="truncate">{account.name}</span>
              {account.scope === "personal" && (
                <span className="text-scope-personal shrink-0">
                  <UserIcon className="size-3.5" aria-hidden />
                  <span className="sr-only">{t("accounts.personal")}</span>
                </span>
              )}
              {account.isArchived && (
                <span className="shrink-0">
                  <ArchiveIcon className="size-3.5" aria-hidden />
                  <span className="sr-only">{t("accounts.archived")}</span>
                </span>
              )}
            </span>
            <span className="text-sm tabular-nums">{balance.display}</span>
            <Button
              variant="ghost"
              size="icon"
              className="size-6 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
              aria-label={t("accounts.edit", { name: account.name })}
              onClick={() => onEdit(account.id)}
            >
              <PencilIcon />
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
