// The /transactions route keeps the account filter in the URL (F5).
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory } from "@tanstack/react-router";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";

import App from "@/App";

const api = vi.hoisted(() => ({
  app: { health: vi.fn(), bootstrap: vi.fn() },
  money: { parse: vi.fn() },
  account: { list: vi.fn() },
  tx: { list: vi.fn() },
}));

vi.mock("@mr-krabs/api-client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@mr-krabs/api-client")>()),
  api,
  transport: "http",
}));

const account = (id: string, name: string) => ({
  account: {
    id,
    name,
    kind: "checking",
    currency: "CLP",
    scope: "household",
    ownerMemberId: null,
    visibility: "shared_full",
    institution: null,
    numberMask: null,
    isArchived: false,
    sortOrder: 0,
  },
  balance: { minor: "1", currency: "CLP", display: "$1" },
});

it("choosing an account writes accountId into the URL, and Todas removes it", async () => {
  api.app.bootstrap.mockResolvedValue({
    household: { id: "h1", name: "Casa Prueba", baseCurrency: "CLP", locale: "es-CL" },
    members: [],
    me: null,
    settings: { baseCurrency: "CLP", locale: "es-CL", budgetMode: "at_purchase" },
    schemaVersion: 3,
  });
  api.account.list.mockResolvedValue([account("a1", "Corriente Prueba"), account("a2", "Ahorro Prueba")]);
  api.tx.list.mockResolvedValue({ items: [], nextCursor: null });
  const history = createMemoryHistory({ initialEntries: ["/transactions"] });
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <App history={history} />
    </QueryClientProvider>,
  );
  const filter = await screen.findByLabelText("Filtrar por cuenta");
  await within(filter).findByRole("option", { name: "Ahorro Prueba" });
  fireEvent.change(filter, { target: { value: "a2" } });
  await waitFor(() => expect(history.location.search).toContain("accountId=a2"));
  await waitFor(() =>
    expect(api.tx.list).toHaveBeenLastCalledWith(expect.objectContaining({ accountId: "a2" })),
  );
  fireEvent.change(filter, { target: { value: "" } });
  await waitFor(() => expect(history.location.search).not.toContain("accountId"));
});
