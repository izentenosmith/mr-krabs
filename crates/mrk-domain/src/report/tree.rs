//! The category tree as reports show it, and additive roll-up through it (DEC-30 Q9).

use std::collections::BTreeMap;

use mrk_money::{CurrencyCode, Money};

use super::{ReportCategory, ReportError};
use crate::category::{CategoryKind, CategoryNode, sibling_key, tree_parents};

/// A category as a report row: its place in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub depth: u32,
    pub kind: CategoryKind,
    pub name: String,
    pub archived: bool,
    /// Has at least one child in the tree.
    pub is_group: bool,
}

/// The live category tree in display order: depth-first, siblings (and roots) ordered by
/// kind (income first), `sort_order`, accent-insensitive name, then id.
///
/// Nothing live is hidden (X-16): a category whose parent is unknown (not among the live
/// rows) is a root, and a parent cycle — possible only through a sync conflict, M1 #29 F3 —
/// is broken by making its smallest member in sibling order a root. `TreeRow::parent_id` is
/// the parent as placed, so it is `None` for every root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryTree {
    rows: Vec<TreeRow>,
    /// Row index by category id.
    index: BTreeMap<String, usize>,
}

impl CategoryTree {
    pub fn new(categories: &[ReportCategory]) -> Self {
        let mut by_id: BTreeMap<&str, &ReportCategory> = BTreeMap::new();
        for c in categories {
            by_id.entry(c.id.as_str()).or_insert(c);
        }
        let nodes: BTreeMap<&str, CategoryNode> = by_id
            .values()
            .map(|c| {
                let node = CategoryNode {
                    id: c.id.clone(),
                    parent_id: c.parent_id.clone(),
                    kind: c.kind,
                    live: true,
                    sort_order: c.sort_order,
                    name: c.name.clone(),
                };
                (c.id.as_str(), node)
            })
            .collect();
        let placed = tree_parents(&nodes.values().cloned().collect::<Vec<_>>());
        let parents: BTreeMap<&str, Option<&str>> = by_id
            .keys()
            .map(|id| (*id, placed.get(*id).and_then(|p| p.as_deref())))
            .collect();
        let mut children: BTreeMap<Option<&str>, Vec<&ReportCategory>> = BTreeMap::new();
        for c in by_id.values() {
            children.entry(parents[c.id.as_str()]).or_default().push(c);
        }
        for list in children.values_mut() {
            list.sort_by_cached_key(|c| {
                let (kind, order, name, id) = sibling_key(&nodes[c.id.as_str()]);
                (kind, order, name, id.to_owned())
            });
        }

        let mut rows = Vec::with_capacity(by_id.len());
        // Depth-first, pre-order: push children in reverse so the first is popped first.
        // Effective parents form a forest, so every category is reached exactly once.
        let mut stack: Vec<(&ReportCategory, u32)> = children
            .get(&None)
            .map(|roots| roots.iter().rev().map(|c| (*c, 0)).collect())
            .unwrap_or_default();
        while let Some((c, depth)) = stack.pop() {
            let kids = children.get(&Some(c.id.as_str()));
            rows.push(TreeRow {
                id: c.id.clone(),
                parent_id: parents[c.id.as_str()].map(str::to_owned),
                depth,
                kind: c.kind,
                name: c.name.clone(),
                archived: c.archived,
                is_group: kids.is_some_and(|k| !k.is_empty()),
            });
            if let Some(kids) = kids {
                stack.extend(kids.iter().rev().map(|k| (*k, depth + 1)));
            }
        }
        let index = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.clone(), i))
            .collect();
        Self { rows, index }
    }

    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    /// The row for `id`, when the category is in the tree.
    pub fn get(&self, id: &str) -> Option<&TreeRow> {
        self.index.get(id).map(|&i| &self.rows[i])
    }

    /// Rolls `own` amounts up the tree: every row = its own amount + all its descendants'.
    /// Every row of the tree is in the result (zero when nothing is under it); amounts on ids
    /// outside the tree are ignored. All amounts must be in `base`.
    pub fn rollup(
        &self,
        own: &BTreeMap<String, Money>,
        base: CurrencyCode,
    ) -> Result<BTreeMap<String, Money>, ReportError> {
        let mut totals: Vec<Money> = self
            .rows
            .iter()
            .map(|r| own.get(&r.id).copied().unwrap_or(Money::zero(base)))
            .collect();
        // Rows are in pre-order, so walking them backwards finishes every subtree before its
        // parent is reached.
        for i in (0..self.rows.len()).rev() {
            if let Some(p) = self.rows[i]
                .parent_id
                .as_deref()
                .and_then(|p| self.index.get(p))
            {
                totals[*p] = totals[*p].checked_add(totals[i])?;
            }
        }
        Ok(self
            .rows
            .iter()
            .zip(totals)
            .map(|(r, t)| (r.id.clone(), t))
            .collect())
    }
}
