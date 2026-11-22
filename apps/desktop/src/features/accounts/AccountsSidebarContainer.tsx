import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiError, api, type AccountInput } from "@mr-krabs/api-client";

import { useBootstrap } from "@/features/shell/bootstrap";
import { isNotImplemented } from "@/lib/api-errors";

import { AccountDialog, type AccountDialogState } from "./AccountDialog";
import { AccountsSidebar, type SidebarState } from "./AccountsSidebar";

export const accountsKey = ["accounts"] as const;

type Editing = null | { mode: "create" } | { mode: "edit"; id: string };

/** Sidebar data and the account dialog. M1 has one member: always the combined view. */
export function AccountsSidebarContainer() {
  const qc = useQueryClient();
  const bootstrap = useBootstrap();
  const [showArchived, setShowArchived] = useState(false);
  const [editing, setEditing] = useState<Editing>(null);

  const list = useQuery({
    queryKey: [...accountsKey, "list", { includeArchived: showArchived }],
    queryFn: () => api.account.list({ view: "combined", includeArchived: showArchived }),
  });

  const editId = editing?.mode === "edit" ? editing.id : null;
  const detail = useQuery({
    queryKey: [...accountsKey, "detail", editId],
    queryFn: () => api.account.get(editId!),
    enabled: editId !== null,
  });

  const done = async () => {
    setEditing(null);
    await qc.invalidateQueries({ queryKey: accountsKey });
  };

  const submit = async (input: AccountInput) => {
    if (editId) await api.account.update(editId, input);
    else await api.account.create(input);
    await done();
  };

  const archive = async () => {
    if (!editId) return;
    await api.account.archive(editId);
    await done();
  };

  return (
    <>
      <AccountsSidebar
        state={sidebarState(list)}
        showArchived={showArchived}
        onToggleArchived={() => setShowArchived((v) => !v)}
        onCreate={() => setEditing({ mode: "create" })}
        onEdit={(id) => setEditing({ mode: "edit", id })}
        onRetry={() => void list.refetch()}
      />
      {editing && (
        <AccountDialog
          key={editId ?? "create"}
          state={
            editing.mode === "create"
              ? { mode: "create", baseCurrency: bootstrap.data?.household?.baseCurrency ?? "CLP" }
              : dialogState(detail)
          }
          onOpenChange={(open) => !open && setEditing(null)}
          onSubmit={submit}
          onArchive={archive}
        />
      )}
    </>
  );
}

type QueryLike<T> = { isPending: boolean; isError: boolean; error: unknown; data: T | undefined };

function sidebarState(q: QueryLike<SidebarAccounts>): SidebarState {
  if (q.isPending) return { status: "loading" };
  if (q.isError) {
    return isNotImplemented(q.error)
      ? { status: "notAvailable" }
      : { status: "error", code: ApiError.from(q.error).code };
  }
  return { status: "ready", accounts: q.data ?? [] };
}

type SidebarAccounts = Awaited<ReturnType<typeof api.account.list>>;
type Detail = Awaited<ReturnType<typeof api.account.get>>;

function dialogState(q: QueryLike<Detail>): AccountDialogState {
  if (q.isPending) return { mode: "edit", status: "loading" };
  if (q.isError) {
    return isNotImplemented(q.error)
      ? { mode: "edit", status: "notAvailable" }
      : { mode: "edit", status: "error", code: ApiError.from(q.error).code };
  }
  return { mode: "edit", status: "ready", detail: q.data! };
}
