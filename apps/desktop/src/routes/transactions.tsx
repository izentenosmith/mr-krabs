import { createRoute, useNavigate, useSearch, type AnyRoute } from "@tanstack/react-router";

import { TransactionsPage, type CategoryOptions } from "@/features/transactions/TransactionsPage";

export interface TransactionsSearch {
  accountId?: string;
}

// Categories come from `category_tree` once household-accounts lands it (DEC-13, REQ-13).
const NO_CATEGORIES: CategoryOptions = { expense: [], income: [] };

function TransactionsRoutePage() {
  const { accountId } = useSearch({ from: "/transactions" });
  const navigate = useNavigate({ from: "/transactions" });
  return (
    <TransactionsPage
      accountId={accountId}
      onAccountIdChange={(id) => void navigate({ search: id ? { accountId: id } : {} })}
      categories={NO_CATEGORIES}
    />
  );
}

/** `/transactions?accountId=<id>`; the shell decides where it hangs. */
export function createTransactionsRoute(getParentRoute: () => AnyRoute) {
  return createRoute({
    getParentRoute,
    path: "/transactions",
    validateSearch: (search: Record<string, unknown>): TransactionsSearch =>
      typeof search.accountId === "string" && search.accountId !== ""
        ? { accountId: search.accountId }
        : {},
    component: TransactionsRoutePage,
  });
}
