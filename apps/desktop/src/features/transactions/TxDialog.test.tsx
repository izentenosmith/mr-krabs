import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ApiError, type AccountDto } from "@mr-krabs/api-client";

import { TxDialog, type TxDialogProps } from "./TxDialog";

const account: AccountDto = {
  id: "a1",
  name: "Corriente Prueba",
  kind: "checking",
  currency: "CLP",
  scope: "household",
  ownerMemberId: null,
  visibility: "shared_full",
  institution: null,
  numberMask: null,
  isArchived: false,
  sortOrder: 0,
};

function renderDialog(props: Partial<TxDialogProps> = {}) {
  const onSubmit = props.onSubmit ?? vi.fn().mockResolvedValue(undefined);
  render(
    <TxDialog
      open
      onOpenChange={() => {}}
      mode="create"
      accounts={[account]}
      categories={{ expense: [{ id: "c1", name: "Supermercado" }], income: [{ id: "c2", name: "Sueldo" }] }}
      onSubmit={onSubmit}
      {...props}
    />,
  );
  return { d: within(screen.getByRole("dialog")), onSubmit };
}

describe("TxDialog", () => {
  it("switching expense → income clears a category the income list doesn't have (F15)", async () => {
    const { d, onSubmit } = renderDialog();
    fireEvent.change(d.getByLabelText("Cuenta"), { target: { value: "a1" } });
    fireEvent.change(d.getByLabelText("Categoría"), { target: { value: "c1" } });
    fireEvent.click(d.getByRole("radio", { name: "Ingreso" }));
    expect(d.getByLabelText("Categoría")).toHaveValue("");
    fireEvent.change(d.getByLabelText("Monto"), { target: { value: "1" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    expect(await d.findByText("Este campo es obligatorio")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Eliminar runs once while a delete is in flight (F13)", async () => {
    let finish!: () => void;
    const onDelete = vi.fn(() => new Promise<void>((r) => (finish = r)));
    const { d } = renderDialog({ mode: "edit", onDelete });
    const button = d.getByRole("button", { name: "Eliminar" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(button).toBeDisabled();
    expect(d.getByRole("button", { name: "Guardar" })).toBeDisabled();
    finish();
    await waitFor(() => expect(button).toBeEnabled());
  });

  it("each type radio carries the MASTER §5 focus ring (F10)", () => {
    const { d } = renderDialog();
    for (const name of ["Gasto", "Ingreso", "Transferencia"]) {
      const label = d.getByRole("radio", { name }).closest("label");
      expect(label?.className).toContain("has-[:focus-visible]:ring-[3px]");
    }
  });

  it("a field error for a control not shown, or an unmapped path, reaches the form-level alert (F16)", async () => {
    const onSubmit = vi
      .fn()
      .mockRejectedValue(new ApiError("VALIDATION", "dev", { categoryId: "errors.field.required" }));
    const { d } = renderDialog({ onSubmit });
    fireEvent.click(d.getByRole("radio", { name: "Transferencia" }));
    fireEvent.change(d.getByLabelText("Desde"), { target: { value: "a1" } });
    fireEvent.change(d.getByLabelText("Hacia"), { target: { value: "a1" } });
    fireEvent.change(d.getByLabelText("Monto"), { target: { value: "1" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    expect(await d.findByRole("alert")).toHaveTextContent("Revisa los datos ingresados.");
  });

  it("a server field value outside errors.field.* shows the generic field text (F17)", async () => {
    const onSubmit = vi.fn().mockRejectedValue(new ApiError("VALIDATION", "dev", { amount: "tx.title" }));
    const { d } = renderDialog({ onSubmit });
    fireEvent.change(d.getByLabelText("Cuenta"), { target: { value: "a1" } });
    fireEvent.change(d.getByLabelText("Categoría"), { target: { value: "c1" } });
    fireEvent.change(d.getByLabelText("Monto"), { target: { value: "1" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    expect(await d.findByText("Revisa este campo")).toBeInTheDocument();
    expect(d.queryByText("Transacciones")).not.toBeInTheDocument();
  });
});
