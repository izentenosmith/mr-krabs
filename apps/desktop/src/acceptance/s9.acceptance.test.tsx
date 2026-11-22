// S9 acceptance tests (REQ-14, DEC-16, MASTER §5.3): per-device ES/EN toggle, through the App.
import { focusManager } from "@tanstack/react-query";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";

import i18n from "@/i18n";
import { loadLanguagePreference } from "@/i18n/language";

import { account, household, noHousehold, renderApp, withHousehold } from "./harness";

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

const KEY = "mrk.uiLanguage";

const english = {
  ...withHousehold,
  household: { ...household, locale: "en" as const },
  settings: { ...withHousehold.settings!, locale: "en" as const },
};

/** A new process on this device: the language module re-reads storage, i18n starts over. */
async function restart() {
  loadLanguagePreference();
  await act(() => i18n.changeLanguage("es-CL"));
}

beforeEach(async () => {
  vi.resetAllMocks();
  vi.restoreAllMocks();
  localStorage.clear();
  await restart();
  api.app.bootstrap.mockResolvedValue(withHousehold);
  api.account.list.mockResolvedValue([account("a-1", "Cuenta Corriente", "checking", "$1.234.567")]);
});

afterEach(async () => {
  vi.restoreAllMocks();
  localStorage.clear();
  await restart();
});

async function languageGroup(name: "Idioma" | "Language") {
  return within(await screen.findByRole("radiogroup", { name }));
}

/** Every api function that writes. Reads (bootstrap, lists, gets) may be called. */
function writes() {
  return [
    api.household.create,
    api.account.create,
    api.account.update,
    api.account.archive,
    api.tx.create,
    api.tx.update,
    api.tx.delete,
    api.tx.restore,
  ];
}

describe("S9 language toggle — sidebar", () => {
  it("AC1: switching to EN re-renders English labels at once, focus stays on the toggle", async () => {
    renderApp();
    const group = await languageGroup("Idioma");
    expect(screen.getByRole("region", { name: "Cuentas" })).toBeInTheDocument();
    expect(group.getByRole("radio", { name: "Español" })).toBeChecked();

    const en = group.getByRole("radio", { name: "English" });
    expect(en).toHaveTextContent("EN");
    en.focus();
    fireEvent.click(en);

    expect(await screen.findByRole("region", { name: "Accounts" })).toBeInTheDocument();
    const after = await languageGroup("Language");
    // Item labels and accessible names are not translated (MASTER §5.3).
    expect(after.getByRole("radio", { name: "English" })).toBeChecked();
    expect(after.getByRole("radio", { name: "Español" })).toHaveTextContent("ES");
    expect(document.activeElement).toBe(after.getByRole("radio", { name: "English" }));
    expect(document.documentElement.lang).toBe("en");
  });

  it("AC1: clicking the active item keeps the language (no deselect)", async () => {
    renderApp();
    const group = await languageGroup("Idioma");
    fireEvent.click(group.getByRole("radio", { name: "Español" }));
    expect(i18n.language).toBe("es-CL");
    expect(group.getByRole("radio", { name: "Español" })).toBeChecked();
  });

  it("AC2: the choice survives an unmount and a restart on this device", async () => {
    const first = renderApp();
    fireEvent.click((await languageGroup("Idioma")).getByRole("radio", { name: "English" }));
    await screen.findByRole("region", { name: "Accounts" });
    first.unmount();

    await restart();
    renderApp();
    expect(await screen.findByRole("region", { name: "Accounts" })).toBeInTheDocument();
    expect((await languageGroup("Language")).getByRole("radio", { name: "English" })).toBeChecked();
  });

  it("AC3: a fresh device starts from the household locale", async () => {
    api.app.bootstrap.mockResolvedValue(english);
    renderApp();
    expect(await screen.findByRole("region", { name: "Accounts" })).toBeInTheDocument();
  });

  it("AC3: a stored choice beats the household locale", async () => {
    localStorage.setItem(KEY, "es-CL");
    await restart();
    api.app.bootstrap.mockResolvedValue(english);
    renderApp();
    expect(await screen.findByRole("region", { name: "Cuentas" })).toBeInTheDocument();
  });

  it("AC3: an unknown stored value is ignored", async () => {
    localStorage.setItem(KEY, "fr");
    await restart();
    api.app.bootstrap.mockResolvedValue(english);
    renderApp();
    expect(await screen.findByRole("region", { name: "Accounts" })).toBeInTheDocument();
  });

  it("AC4: storage that throws doesn't break the app; the choice holds for this run", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("quota");
    });
    await restart();

    renderApp();
    fireEvent.click((await languageGroup("Idioma")).getByRole("radio", { name: "English" }));
    expect(await screen.findByRole("region", { name: "Accounts" })).toBeInTheDocument();

    // A bootstrap refetch (window focus) doesn't revert the run's choice to the household's.
    const before = api.app.bootstrap.mock.calls.length;
    act(() => {
      focusManager.setFocused(false);
      focusManager.setFocused(true);
    });
    await waitFor(() => expect(api.app.bootstrap.mock.calls.length).toBeGreaterThan(before));
    focusManager.setFocused(undefined);
    expect(await screen.findByRole("region", { name: "Accounts" })).toBeInTheDocument();
  });

  it("AC6: switching writes nothing through the api; money and names render as sent", async () => {
    renderApp();
    const region = await screen.findByRole("region", { name: "Cuentas" });
    expect(await within(region).findByText("$1.234.567")).toBeInTheDocument();

    fireEvent.click((await languageGroup("Idioma")).getByRole("radio", { name: "English" }));
    const en = await screen.findByRole("region", { name: "Accounts" });
    expect(within(en).getByText("$1.234.567")).toBeInTheDocument();
    expect(within(en).getByText("Cuenta Corriente")).toBeInTheDocument();
    fireEvent.click((await languageGroup("Language")).getByRole("radio", { name: "Español" }));
    await screen.findByRole("region", { name: "Cuentas" });

    for (const fn of writes()) expect(fn).not.toHaveBeenCalled();
    expect(localStorage.getItem(KEY)).toBe("es-CL");
  });
});

describe("S9 language toggle — onboarding", () => {
  it("AC5: the onboarding toggle switches before typing and becomes the stored choice", async () => {
    api.app.bootstrap.mockResolvedValueOnce(noHousehold).mockResolvedValue(withHousehold);
    api.household.create.mockResolvedValue(household);
    renderApp();

    await screen.findByRole("heading", { name: "Te damos la bienvenida a Mr Krabs" });
    fireEvent.click((await languageGroup("Idioma")).getByRole("radio", { name: "English" }));
    expect(await screen.findByRole("heading", { name: "Welcome to Mr Krabs" })).toBeInTheDocument();
    expect(localStorage.getItem(KEY)).toBe("en");

    fireEvent.change(screen.getByLabelText("Household name"), { target: { value: "Casa Prueba" } });
    fireEvent.change(screen.getByLabelText("Your name"), { target: { value: "Persona Prueba" } });
    fireEvent.click(screen.getByRole("button", { name: "Get started" }));

    // The household is es-CL, but this device keeps its choice; create sends no locale.
    expect(await screen.findByRole("region", { name: "Accounts" })).toBeInTheDocument();
    expect(api.household.create).toHaveBeenCalledWith({
      name: "Casa Prueba",
      myName: "Persona Prueba",
      baseCurrency: "CLP",
    });
  });
});
