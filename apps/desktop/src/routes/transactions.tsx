import { createRoute } from "@tanstack/react-router";

import { TransactionsPage } from "@/features/transactions/TransactionsPage";
import { appLayoutRoute } from "@/routes/tree";

export interface TransactionsSearch {
  accountId?: string;
}

function TransactionsRoutePage() {
  const { accountId } = transactionsRoute.useSearch();
  const navigate = transactionsRoute.useNavigate();
  return (
    <TransactionsPage
      accountId={accountId}
      onAccountIdChange={(id) => void navigate({ search: id ? { accountId: id } : {} })}
    />
  );
}

/** `/transactions?accountId=<id>`, a screen inside the app shell (S6's layout route). */
export const transactionsRoute = createRoute({
  getParentRoute: () => appLayoutRoute,
  path: "/transactions",
  validateSearch: (search: Record<string, unknown>): TransactionsSearch =>
    typeof search.accountId === "string" && search.accountId !== ""
      ? { accountId: search.accountId }
      : {},
  component: TransactionsRoutePage,
});
