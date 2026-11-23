// Acceptance harness for S6: renders the whole App (router + query client) against a mocked
// `api`. Tests act only through what a user sees (labels, roles, text).
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory } from "@tanstack/react-router";
import { render } from "@testing-library/react";
import type {
  AccountDetailDto,
  AccountDto,
  AccountKind,
  AccountSummaryDto,
  BootstrapDto,
  HouseholdDto,
} from "@mr-krabs/api-client";

import App from "@/App";

export const household: HouseholdDto = {
  id: "00000000-0000-7000-8000-000000000001",
  name: "Casa Prueba",
  baseCurrency: "CLP",
  locale: "es-CL",
};

export const noHousehold: BootstrapDto = {
  household: null,
  members: [],
  me: null,
  settings: null,
  schemaVersion: 3,
};

export const withHousehold: BootstrapDto = {
  household,
  members: [{ id: "m-1", displayName: "Persona Prueba", color: null }],
  me: { id: "m-1", displayName: "Persona Prueba", color: null },
  settings: { baseCurrency: "CLP", locale: "es-CL", budgetMode: "at_purchase" },
  schemaVersion: 3,
};

export function account(
  id: string,
  name: string,
  kind: AccountKind,
  display: string,
  extra: Partial<AccountDto> = {},
): AccountSummaryDto {
  return {
    account: {
      id,
      name,
      kind,
      currency: extra.currency ?? "CLP",
      scope: "household",
      ownerMemberId: null,
      visibility: "shared_full",
      institution: null,
      numberMask: null,
      isArchived: false,
      sortOrder: 0,
      ...extra,
    },
    // `minor` deliberately disagrees with `display`: the UI must show `display` verbatim.
    balance: { minor: "1", currency: extra.currency ?? "CLP", display },
  };
}

export function detail(summary: AccountSummaryDto, opening: AccountDetailDto["openingBalance"]) {
  return { account: summary.account, balance: summary.balance, openingBalance: opening };
}

export function renderApp(path = "/") {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <App history={createMemoryHistory({ initialEntries: [path] })} />
    </QueryClientProvider>,
  );
}
