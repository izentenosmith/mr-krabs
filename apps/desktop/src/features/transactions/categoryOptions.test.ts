import type { CategoryNodeDto } from "@mr-krabs/api-client";

import { categoryOptions } from "./categoryOptions";

const n = (id: string, name: string, children: CategoryNodeDto[] = [], extra: Partial<CategoryNodeDto> = {}) =>
  ({ id, parentId: null, kind: "expense", name, icon: null, isArchived: false, sortOrder: 0, children, ...extra }) as CategoryNodeDto;

describe("categoryOptions", () => {
  it("groups descendants under their root, names deeper ones by path, drops archived subtrees, sorts", () => {
    const tree = [
      n("t", "Transporte", [], { sortOrder: 2 }),
      n("h", "Hogar", [
        n("h2", "Servicios", [n("h21", "Luz"), n("h22", "Agua Archivada", [], { isArchived: true })], { sortOrder: 1 }),
        n("h1", "Arriendo", [], { sortOrder: 0 }),
        n("hx", "Vieja", [n("hx1", "Bajo archivada")], { isArchived: true }),
      ], { sortOrder: 1 }),
      n("z", "Archivada raíz", [], { isArchived: true }),
    ];
    expect(categoryOptions(tree)).toEqual([
      { id: "h1", name: "Arriendo", group: "Hogar", groupId: "h" },
      { id: "h2", name: "Servicios", group: "Hogar", groupId: "h" },
      { id: "h21", name: "Servicios › Luz", group: "Hogar", groupId: "h" },
      { id: "t", name: "Transporte" },
    ]);
  });

  it("a root whose children are all archived is a plain option", () => {
    expect(categoryOptions([n("r", "Ocio", [n("a", "X", [], { isArchived: true })])])).toEqual([
      { id: "r", name: "Ocio" },
    ]);
  });
});

describe("categoryOptions groups", () => {
  it("keys each group by its root's id, so two roots with the same name stay apart (X-13 F6)", () => {
    const opts = categoryOptions([
      n("r1", "Hogar", [n("a", "Arriendo")]),
      n("r2", "Hogar", [n("b", "Luz")]),
    ]);
    expect(opts.map((o) => o.groupId)).toEqual(["r1", "r2"]);
  });
});
