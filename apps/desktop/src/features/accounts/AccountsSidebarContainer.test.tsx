// Regression tests from tyr-verdict iteration 1 (.workflow/ledger.md), through the App.
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import type { AccountSummaryDto } from "@mr-krabs/api-client";

import { account, detail, renderApp, withHousehold } from "@/acceptance/harness";

const api = vi.hoisted(() => ({
  app: { health: vi.fn(), bootstrap: vi.fn() },
  household: { create: vi.fn() },
  money: { parse: vi.fn() },
  account: { list: vi.fn(), get: vi.fn(), create: vi.fn(), update: vi.fn(), archive: vi.fn() },
}));

vi.mock("@mr-krabs/api-client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@mr-krabs/api-client")>()),
  transport: "http",
  api,
}));

beforeEach(() => {
  vi.resetAllMocks();
  api.app.health.mockResolvedValue({
    status: "ok",
    version: "0.1.0",
    schemaVersion: 3,
    latestSchemaVersion: 3,
    dbPath: null,
  });
  api.app.bootstrap.mockResolvedValue(withHousehold);
});

describe("accounts sidebar", () => {
  it("F5: toggling archived keeps the current rows until the new list arrives", async () => {
    const live = account("a-1", "Corriente", "checking", "$0");
    const old = account("a-2", "Cerrada", "checking", "$0", { isArchived: true });
    let release: (rows: AccountSummaryDto[]) => void = () => {};
    api.account.list.mockImplementation(async (q: { includeArchived?: boolean }) =>
      q.includeArchived ? new Promise((r) => (release = r)) : [live],
    );
    renderApp();
    const s = within(await screen.findByRole("region", { name: "Cuentas" }));
    await s.findByText("Corriente");

    fireEvent.click(s.getByRole("button", { name: "Mostrar archivadas" }));
    await waitFor(() =>
      expect(api.account.list).toHaveBeenLastCalledWith({ view: "combined", includeArchived: true }),
    );
    expect(s.getByText("Corriente")).toBeInTheDocument();
    expect(s.queryByText("Cargando…")).not.toBeInTheDocument();

    release([live, old]);
    expect(await s.findByText("Cerrada")).toBeInTheDocument();
  });

  it("X-8/X-9: editing a EUR account re-sends its opening balance display text verbatim", async () => {
    const eur = account("a-eur", "Cuenta Euro", "savings", "€1.234,56", { currency: "EUR" });
    api.account.list.mockResolvedValue([eur]);
    api.account.get.mockResolvedValue(
      detail(eur, { amount: { minor: "123456", currency: "EUR", display: "€1.234,56" }, date: "2026-11-01" }),
    );
    api.account.update.mockResolvedValue(eur.account);
    renderApp();
    const s = within(await screen.findByRole("region", { name: "Cuentas" }));
    fireEvent.click(await s.findByRole("button", { name: "Editar Cuenta Euro" }));
    const d = within(await screen.findByRole("dialog", { name: "Editar cuenta" }));
    await waitFor(() =>
      expect((d.getByLabelText("Saldo inicial") as HTMLInputElement).value).toBe("€1.234,56"),
    );

    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() =>
      expect(api.account.update).toHaveBeenCalledWith(
        "a-eur",
        expect.objectContaining({
          currency: "EUR",
          openingBalance: { amount: { amount: "€1.234,56", currency: "EUR" }, date: "2026-11-01" },
        }),
      ),
    );
  });
});
