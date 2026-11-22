import { render, screen } from "@testing-library/react";

import { account, detail } from "@/acceptance/harness";

import { AccountDialog } from "./AccountDialog";

function renderEdit(isArchived: boolean) {
  const summary = account("a-1", "Cuenta Prueba", "checking", "$0", { isArchived });
  render(
    <AccountDialog
      state={{ mode: "edit", status: "ready", detail: detail(summary, null) }}
      onOpenChange={vi.fn()}
      onSubmit={vi.fn()}
      onArchive={vi.fn()}
    />,
  );
}

describe("AccountDialog", () => {
  it("F4: a live account's edit dialog offers Archivar", () => {
    renderEdit(false);
    expect(screen.getByRole("button", { name: "Archivar" })).toBeInTheDocument();
  });

  it("F4: hides Archivar for an account that is already archived", () => {
    renderEdit(true);
    expect(screen.getByRole("button", { name: "Guardar" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Archivar" })).not.toBeInTheDocument();
  });
});
