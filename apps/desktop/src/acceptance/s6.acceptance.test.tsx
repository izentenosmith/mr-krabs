// S6 acceptance tests — one per criterion in .workflow/brief.md, through the App boundary.
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { ApiError } from "@mr-krabs/api-client";

import i18n from "@/i18n";

import {
  account,
  detail,
  household,
  noHousehold,
  renderApp,
  withHousehold,
} from "./harness";

const api = vi.hoisted(() => ({
  app: { health: vi.fn(), bootstrap: vi.fn() },
  household: { create: vi.fn() },
  settings: { get: vi.fn() },
  money: { parse: vi.fn() },
  account: {
    list: vi.fn(),
    get: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    archive: vi.fn(),
  },
  tx: {
    create: vi.fn(),
    get: vi.fn(),
    list: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    restore: vi.fn(),
  },
}));

vi.mock("@mr-krabs/api-client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@mr-krabs/api-client")>()),
  transport: "http",
  api,
}));

const notImplemented = (cmd: string) => new ApiError("NOT_IMPLEMENTED", `${cmd} not implemented`);

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
  api.account.list.mockResolvedValue([]);
});

function fill(label: string | RegExp, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

async function sidebar() {
  return within(await screen.findByRole("region", { name: "Cuentas" }));
}

async function openNewAccount() {
  const s = await sidebar();
  fireEvent.click(s.getAllByRole("button", { name: "Nueva cuenta" })[0]);
  return within(await screen.findByRole("dialog", { name: "Nueva cuenta" }));
}

describe("S6 onboarding", () => {
  it("tracer: onboarding creates the household in CLP and then shows the shell", async () => {
    api.app.bootstrap.mockResolvedValueOnce(noHousehold).mockResolvedValue(withHousehold);
    api.household.create.mockResolvedValue(household);
    renderApp();

    expect(
      await screen.findByRole("heading", { name: "Te damos la bienvenida a Mr Krabs" }),
    ).toBeInTheDocument();
    fill("Nombre del hogar", "Casa Prueba");
    fill("Tu nombre", "Persona Prueba");
    fireEvent.click(screen.getByRole("button", { name: "Comenzar" }));

    await waitFor(() =>
      expect(api.household.create).toHaveBeenCalledWith({
        name: "Casa Prueba",
        myName: "Persona Prueba",
        baseCurrency: "CLP",
      }),
    );
    expect(await screen.findByRole("region", { name: "Cuentas" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Comenzar" })).not.toBeInTheDocument();
  });

  it("offers exactly CLP, USD, EUR and CLF with CLP selected; USD is sent when chosen", async () => {
    api.app.bootstrap.mockResolvedValue(noHousehold);
    api.household.create.mockResolvedValue({ ...household, baseCurrency: "USD" });
    renderApp();

    const select = (await screen.findByLabelText("Moneda base")) as HTMLSelectElement;
    expect(Array.from(select.options).map((o) => o.value)).toEqual(["CLP", "USD", "EUR", "CLF"]);
    expect(select.value).toBe("CLP");

    fill("Nombre del hogar", "Casa Prueba");
    fill("Tu nombre", "Persona Prueba");
    fireEvent.change(select, { target: { value: "USD" } });
    fireEvent.click(screen.getByRole("button", { name: "Comenzar" }));
    await waitFor(() =>
      expect(api.household.create).toHaveBeenCalledWith(
        expect.objectContaining({ baseCurrency: "USD" }),
      ),
    );
  });
});

describe("S6 accounts sidebar", () => {
  it("shows names and balance display strings verbatim, grouped by kind in MASTER order", async () => {
    api.account.list.mockResolvedValue([
      account("a-card", "Visa Prueba", "credit_card", "-$45.990"),
      account("a-chk", "Corriente Prueba", "checking", "$500.000"),
      account("a-usd", "Ahorro Dólares", "savings", "US$1.234,56", { currency: "USD" }),
      account("a-inv", "Fondo Prueba", "investment", "$0"),
    ]);
    renderApp();
    const s = await sidebar();

    expect(await s.findByText("$500.000")).toBeInTheDocument();
    expect(s.getByText("-$45.990")).toBeInTheDocument();
    expect(s.getByText("US$1.234,56")).toBeInTheDocument();
    expect(s.getByText("Corriente Prueba")).toBeInTheDocument();
    // No client formatting of `minor` ("1") anywhere.
    expect(s.queryByText(/\$1$/)).not.toBeInTheDocument();

    const captions = ["Bancos y efectivo", "Tarjetas de crédito", "Créditos", "Inversiones", "Otros"]
      .map((c) => s.queryByText(c))
      .filter((el): el is HTMLElement => el !== null)
      .map((el) => el.textContent);
    expect(captions).toEqual(["Bancos y efectivo", "Tarjetas de crédito", "Inversiones"]);

    const bank = s.getByRole("group", { name: "Bancos y efectivo" });
    expect(within(bank).getByText("Corriente Prueba")).toBeInTheDocument();
    expect(within(bank).getByText("Ahorro Dólares")).toBeInTheDocument();
    expect(within(s.getByRole("group", { name: "Tarjetas de crédito" })).getByText("Visa Prueba"))
      .toBeInTheDocument();
    expect(api.account.list).toHaveBeenCalledWith({ view: "combined", includeArchived: false });
  });

  it("the archived toggle re-queries with includeArchived and marks archived accounts", async () => {
    api.account.list.mockImplementation(async (q: { includeArchived?: boolean }) =>
      q.includeArchived
        ? [
            account("a-chk", "Corriente Prueba", "checking", "$500.000"),
            account("a-old", "Cuenta Cerrada", "checking", "$0", { isArchived: true }),
          ]
        : [account("a-chk", "Corriente Prueba", "checking", "$500.000")],
    );
    renderApp();
    const s = await sidebar();
    await s.findByText("Corriente Prueba");
    expect(s.queryByText("Cuenta Cerrada")).not.toBeInTheDocument();

    fireEvent.click(s.getByRole("button", { name: "Mostrar archivadas" }));
    expect(await s.findByText("Cuenta Cerrada")).toBeInTheDocument();
    expect(api.account.list).toHaveBeenLastCalledWith({ view: "combined", includeArchived: true });
    expect(s.getByText("Archivada")).toBeInTheDocument();

    fireEvent.click(s.getByRole("button", { name: "Ocultar archivadas" }));
    await waitFor(() => expect(s.queryByText("Cuenta Cerrada")).not.toBeInTheDocument());
    expect(api.account.list).toHaveBeenLastCalledWith({ view: "combined", includeArchived: false });
  });
});

describe("S6 account dialog", () => {
  it("creates an account with the opening balance as typed text and refreshes the list", async () => {
    api.account.list
      .mockResolvedValueOnce([])
      .mockResolvedValue([account("a-new", "Corriente Prueba", "checking", "$500.000")]);
    api.account.create.mockResolvedValue(
      account("a-new", "Corriente Prueba", "checking", "$500.000").account,
    );
    renderApp();
    const d = await openNewAccount();

    expect((d.getByLabelText("Moneda") as HTMLSelectElement).value).toBe("CLP");
    fireEvent.change(d.getByLabelText("Nombre"), { target: { value: "Corriente Prueba" } });
    fireEvent.change(d.getByLabelText("Tipo"), { target: { value: "checking" } });
    fireEvent.change(d.getByLabelText("Saldo inicial"), { target: { value: "500.000" } });
    fireEvent.change(d.getByLabelText("Fecha del saldo inicial"), {
      target: { value: "2026-11-01" },
    });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));

    await waitFor(() =>
      expect(api.account.create).toHaveBeenCalledWith({
        name: "Corriente Prueba",
        kind: "checking",
        currency: "CLP",
        institution: null,
        numberMask: null,
        openingBalance: { amount: { amount: "500.000", currency: "CLP" }, date: "2026-11-01" },
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await (await sidebar()).findByText("$500.000")).toBeInTheDocument();
  });

  it("creates an account without opening balance and with empty optional texts as null", async () => {
    api.account.create.mockResolvedValue(account("a-1", "Billetera", "cash", "$0").account);
    renderApp();
    const d = await openNewAccount();

    fireEvent.change(d.getByLabelText("Nombre"), { target: { value: "Billetera" } });
    fireEvent.change(d.getByLabelText("Tipo"), { target: { value: "cash" } });
    fireEvent.change(d.getByLabelText("Institución"), { target: { value: "   " } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));

    await waitFor(() =>
      expect(api.account.create).toHaveBeenCalledWith({
        name: "Billetera",
        kind: "cash",
        currency: "CLP",
        institution: null,
        numberMask: null,
        openingBalance: null,
      }),
    );
  });

  it("edits an account prefilled from account_get, re-sending the opening balance display text", async () => {
    const chk = account("a-chk", "Corriente Prueba", "checking", "$455.000", {
      scope: "personal",
      ownerMemberId: "m-1",
      visibility: "shared_summary",
      institution: "Banco Prueba",
      numberMask: "•••• 1234",
    });
    api.account.list.mockResolvedValue([chk]);
    api.account.get.mockResolvedValue(
      detail(chk, {
        amount: { minor: "500000", currency: "CLP", display: "$500.000" },
        date: "2026-11-01",
      }),
    );
    api.account.update.mockResolvedValue({ ...chk.account, name: "Corriente Principal" });
    renderApp();
    const s = await sidebar();
    fireEvent.click(await s.findByRole("button", { name: "Editar Corriente Prueba" }));

    const d = within(await screen.findByRole("dialog", { name: "Editar cuenta" }));
    await waitFor(() =>
      expect((d.getByLabelText("Nombre") as HTMLInputElement).value).toBe("Corriente Prueba"),
    );
    expect((d.getByLabelText("Saldo inicial") as HTMLInputElement).value).toBe("$500.000");
    expect(api.account.get).toHaveBeenCalledWith("a-chk");

    fireEvent.change(d.getByLabelText("Nombre"), { target: { value: "Corriente Principal" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));

    await waitFor(() =>
      expect(api.account.update).toHaveBeenCalledWith("a-chk", {
        name: "Corriente Principal",
        kind: "checking",
        currency: "CLP",
        scope: "personal",
        visibility: "shared_summary",
        institution: "Banco Prueba",
        numberMask: "•••• 1234",
        openingBalance: { amount: { amount: "$500.000", currency: "CLP" }, date: "2026-11-01" },
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("archives from the edit dialog, refreshes the list and closes", async () => {
    const chk = account("a-chk", "Corriente Prueba", "checking", "$0");
    api.account.list.mockResolvedValueOnce([chk]).mockResolvedValue([]);
    api.account.get.mockResolvedValue(detail(chk, null));
    api.account.archive.mockResolvedValue({ ...chk.account, isArchived: true });
    renderApp();
    const s = await sidebar();
    fireEvent.click(await s.findByRole("button", { name: "Editar Corriente Prueba" }));
    const d = within(await screen.findByRole("dialog", { name: "Editar cuenta" }));
    await waitFor(() =>
      expect((d.getByLabelText("Nombre") as HTMLInputElement).value).toBe("Corriente Prueba"),
    );

    fireEvent.click(d.getByRole("button", { name: "Archivar" }));
    await waitFor(() => expect(api.account.archive).toHaveBeenCalledWith("a-chk"));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(s.queryByText("Corriente Prueba")).not.toBeInTheDocument());
  });
});

describe("S6 shell", () => {
  it("renders the existing home page inside the layout outlet, next to the sidebar", async () => {
    renderApp("/");
    expect(await screen.findByRole("region", { name: "Cuentas" })).toBeInTheDocument();
    const main = screen.getByRole("main");
    expect(await within(main).findByText("En línea")).toBeInTheDocument();
    expect(within(main).getByText("en memoria")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Principal" })).toBeInTheDocument();
  });
});

describe("S6 edge cases", () => {
  it("required fields empty → shape errors, no api call", async () => {
    api.app.bootstrap.mockResolvedValue(noHousehold);
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "Comenzar" }));
    expect(await screen.findAllByText("Este campo es obligatorio")).toHaveLength(2);
    expect(api.household.create).not.toHaveBeenCalled();
    cleanup();

    api.app.bootstrap.mockResolvedValue(withHousehold);
    api.account.list.mockResolvedValue([]);
    renderApp();
    const d = await openNewAccount();
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    expect(await d.findByText("Este campo es obligatorio")).toBeInTheDocument();
    expect(api.account.create).not.toHaveBeenCalled();
  });

  it("VALIDATION fields map to form errors; an unknown key shows the generic message", async () => {
    api.app.bootstrap.mockResolvedValue(noHousehold);
    api.household.create.mockRejectedValue(
      new ApiError("VALIDATION", "bad", { name: "errors.field.tooLong", myName: "errors.field.nope" }),
    );
    renderApp();
    await screen.findByRole("button", { name: "Comenzar" });
    fill("Nombre del hogar", "x");
    fill("Tu nombre", "y");
    fireEvent.click(screen.getByRole("button", { name: "Comenzar" }));
    expect(await screen.findByText("Es demasiado largo")).toBeInTheDocument();
    expect(screen.getByText("Revisa este campo")).toBeInTheDocument();
    expect(screen.getByLabelText("Nombre del hogar")).toHaveAttribute("aria-invalid", "true");
    cleanup();

    api.app.bootstrap.mockResolvedValue(withHousehold);
    api.account.list.mockResolvedValue([]);
    api.account.create.mockRejectedValue(
      new ApiError("VALIDATION", "bad", {
        name: "errors.field.required",
        "openingBalance.amount": "errors.field.invalidAmount",
      }),
    );
    renderApp();
    const d = await openNewAccount();
    fireEvent.change(d.getByLabelText("Nombre"), { target: { value: " x " } });
    fireEvent.change(d.getByLabelText("Saldo inicial"), { target: { value: "12,3,4" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    expect(await d.findByText("Monto no válido")).toBeInTheDocument();
    expect(d.getByText("Este campo es obligatorio")).toBeInTheDocument();
    expect(d.getByLabelText("Saldo inicial")).toHaveAttribute("aria-invalid", "true");
  });

  it.each([
    ["VALIDATION", "Revisa los datos ingresados."],
    ["CURRENCY_MISMATCH", "La moneda no coincide con la de la cuenta."],
    ["CONFLICT", "Esto entra en conflicto con datos existentes."],
    ["INTERNAL", "Algo salió mal. Intenta de nuevo."],
    ["TRANSPORT", "No pudimos conectar con el núcleo. Intenta de nuevo."],
  ] as const)("%s without fields → form-level alert with mapped copy", async (code, copy) => {
    api.account.create.mockRejectedValue(new ApiError(code, "developer message"));
    renderApp();
    const d = await openNewAccount();
    fireEvent.change(d.getByLabelText("Nombre"), { target: { value: "Corriente" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    expect(await d.findByRole("alert")).toHaveTextContent(copy);
    expect(d.queryByText("developer message")).not.toBeInTheDocument();
    expect(d.queryByText(code)).not.toBeInTheDocument();
    expect((d.getByLabelText("Nombre") as HTMLInputElement).value).toBe("Corriente");
  });

  it("account_list NOT_IMPLEMENTED → not-yet-available in the sidebar, not an error", async () => {
    api.account.list.mockRejectedValue(notImplemented("account_list"));
    renderApp();
    const s = await sidebar();
    expect(await s.findByText("Aún no disponible")).toBeInTheDocument();
    expect(s.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("create NOT_IMPLEMENTED → not-yet-available message in the form, values kept", async () => {
    api.app.bootstrap.mockResolvedValue(noHousehold);
    api.household.create.mockRejectedValue(notImplemented("household_create"));
    renderApp();
    await screen.findByRole("button", { name: "Comenzar" });
    fill("Nombre del hogar", "Casa Prueba");
    fill("Tu nombre", "Persona Prueba");
    fireEvent.click(screen.getByRole("button", { name: "Comenzar" }));
    expect(await screen.findByText("Aún no disponible")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect((screen.getByLabelText("Nombre del hogar") as HTMLInputElement).value).toBe(
      "Casa Prueba",
    );
    cleanup();

    api.app.bootstrap.mockResolvedValue(withHousehold);
    api.account.list.mockResolvedValue([]);
    api.account.create.mockRejectedValue(notImplemented("account_create"));
    renderApp();
    const d = await openNewAccount();
    fireEvent.change(d.getByLabelText("Nombre"), { target: { value: "Corriente" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    expect(await d.findByText("Aún no disponible")).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Nueva cuenta" })).toBeInTheDocument();
    expect((d.getByLabelText("Nombre") as HTMLInputElement).value).toBe("Corriente");
  });

  it("bootstrap NOT_IMPLEMENTED → app not-yet-available; other errors → retry", async () => {
    api.app.bootstrap.mockRejectedValue(notImplemented("app_bootstrap"));
    renderApp();
    expect(await screen.findByText("Aún no disponible")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Cuentas" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Comenzar" })).not.toBeInTheDocument();
    cleanup();

    api.app.bootstrap
      .mockRejectedValueOnce(new ApiError("TRANSPORT", "down"))
      .mockResolvedValue(withHousehold);
    api.account.list.mockResolvedValue([]);
    renderApp();
    expect(await screen.findByRole("alert")).toHaveTextContent("No pudimos iniciar la app");
    fireEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    expect(await screen.findByRole("region", { name: "Cuentas" })).toBeInTheDocument();
  });

  it("an empty account list invites creating the first account", async () => {
    renderApp();
    const s = await sidebar();
    expect(await s.findByText("Crea tu primera cuenta")).toBeInTheDocument();
    const buttons = s.getAllByRole("button", { name: "Nueva cuenta" });
    fireEvent.click(buttons[buttons.length - 1]);
    expect(await screen.findByRole("dialog", { name: "Nueva cuenta" })).toBeInTheDocument();
  });
});

describe("S6 copy", () => {
  afterEach(async () => {
    await i18n.changeLanguage("es-CL");
  });

  it("renders the onboarding in English when the language is en", async () => {
    await i18n.changeLanguage("en");
    api.app.bootstrap.mockResolvedValue(noHousehold);
    renderApp();
    expect(await screen.findByRole("heading", { name: "Welcome to Mr Krabs" })).toBeInTheDocument();
    expect(screen.getByLabelText("Household name")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Get started" })).toBeInTheDocument();
  });
});

describe("S6 journey", () => {
  it("fresh install: onboarding in CLP, create a checking account, see its balance", async () => {
    api.app.bootstrap.mockResolvedValueOnce(noHousehold).mockResolvedValue(withHousehold);
    api.household.create.mockResolvedValue(household);
    const chk = account("a-chk", "Cuenta Corriente", "checking", "$1.000.000");
    api.account.list.mockResolvedValueOnce([]).mockResolvedValue([chk]);
    api.account.create.mockResolvedValue(chk.account);
    renderApp();

    await screen.findByRole("button", { name: "Comenzar" });
    fill("Nombre del hogar", "Casa Prueba");
    fill("Tu nombre", "Persona Prueba");
    fireEvent.click(screen.getByRole("button", { name: "Comenzar" }));

    const d = await openNewAccount();
    fireEvent.change(d.getByLabelText("Nombre"), { target: { value: "Cuenta Corriente" } });
    fireEvent.change(d.getByLabelText("Saldo inicial"), { target: { value: "1.000.000" } });
    fireEvent.change(d.getByLabelText("Fecha del saldo inicial"), {
      target: { value: "2026-11-01" },
    });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));

    const s = await sidebar();
    expect(await s.findByText("$1.000.000")).toBeInTheDocument();
    expect(s.getByText("Cuenta Corriente")).toBeInTheDocument();
  });
});
