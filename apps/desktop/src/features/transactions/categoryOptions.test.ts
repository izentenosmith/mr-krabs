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
      { id: "h1", name: "Arriendo", group: "Hogar" },
      { id: "h2", name: "Servicios", group: "Hogar" },
      { id: "h21", name: "Servicios › Luz", group: "Hogar" },
      { id: "t", name: "Transporte" },
    ]);
  });

  it("a root whose children are all archived is a plain option", () => {
    expect(categoryOptions([n("r", "Ocio", [n("a", "X", [], { isArchived: true })])])).toEqual([
      { id: "r", name: "Ocio" },
    ]);
  });
});
