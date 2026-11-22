import type { CategoryNodeDto } from "@mr-krabs/api-client";

import type { CategoryOption } from "./TxDialog";

const bySortOrder = (a: CategoryNodeDto, b: CategoryNodeDto) => a.sortOrder - b.sortOrder;
const live = (nodes: CategoryNodeDto[]) => nodes.filter((n) => !n.isArchived).sort(bySortOrder);

/**
 * The picker's options for one kind's live tree (REQ-13): a root with live children is a
 * group of its descendants (deeper ones named "Padre › Hijo"), a childless root is a plain
 * option. Archived nodes and their subtrees are left out.
 */
export function categoryOptions(tree: CategoryNodeDto[]): CategoryOption[] {
  const out: CategoryOption[] = [];
  for (const root of live(tree)) {
    const children = live(root.children);
    if (children.length === 0) {
      out.push({ id: root.id, name: root.name });
      continue;
    }
    const walk = (nodes: CategoryNodeDto[], prefix: string) => {
      for (const n of live(nodes)) {
        const name = prefix ? `${prefix} › ${n.name}` : n.name;
        out.push({ id: n.id, name, group: root.name });
        walk(n.children, name);
      }
    };
    walk(children, "");
  }
  return out;
}
