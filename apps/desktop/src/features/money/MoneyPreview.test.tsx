import { fireEvent, render, screen } from "@testing-library/react";

import { MoneyPreview } from "./MoneyPreview";

describe("MoneyPreview", () => {
  it("shows the backend's formatted result", async () => {
    const onParse = vi.fn().mockResolvedValue({ minor: "27990", currency: "CLP", display: "$27.990" });
    render(<MoneyPreview onParse={onParse} />);

    fireEvent.change(screen.getByLabelText("Monto"), { target: { value: "27.990" } });
    fireEvent.click(screen.getByRole("button", { name: "Interpretar" }));

    expect(await screen.findByText("$27.990")).toBeInTheDocument();
    expect(onParse).toHaveBeenCalledWith("27.990");
  });

  it("shows backend validation errors", async () => {
    const onParse = vi.fn().mockRejectedValue(new Error("invalid amount"));
    render(<MoneyPreview onParse={onParse} />);
    fireEvent.click(screen.getByRole("button", { name: "Interpretar" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("invalid amount");
  });
});
