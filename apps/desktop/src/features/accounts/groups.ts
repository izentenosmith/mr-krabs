import type { AccountKind, AccountSummaryDto } from "@mr-krabs/api-client";

/** Sidebar groups in MASTER §5.1 order; every AccountKind belongs to exactly one. */
export const ACCOUNT_GROUPS = [
  { id: "banks", kinds: ["checking", "cuenta_vista", "savings", "cash"] },
  { id: "cards", kinds: ["credit_card"] },
  { id: "loans", kinds: ["loan", "mortgage"] },
  { id: "investments", kinds: ["investment"] },
  { id: "other", kinds: ["other_asset", "other_liability"] },
] as const satisfies readonly { id: string; kinds: readonly AccountKind[] }[];

export type AccountGroupId = (typeof ACCOUNT_GROUPS)[number]["id"];

/** Kinds in dialog order (the groups' order). */
export const ACCOUNT_KINDS: AccountKind[] = ACCOUNT_GROUPS.flatMap((g) => [...g.kinds]);

/** Non-empty groups, each keeping the order `account_list` returned. */
export function groupAccounts(accounts: AccountSummaryDto[]) {
  return ACCOUNT_GROUPS.map((g) => ({
    id: g.id as AccountGroupId,
    accounts: accounts.filter((a) => (g.kinds as readonly AccountKind[]).includes(a.account.kind)),
  })).filter((g) => g.accounts.length > 0);
}
