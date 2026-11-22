//! `category_tree` (REQ-13, DEC-13): the live category tree, read-only.

use std::collections::{BTreeMap, HashSet};

use mrk_store::CategoryRow;

use crate::api::{CategoryNodeDto, CategoryTreeQuery};
use crate::support::{command, from_wire, sort_order, wire};
use crate::{ApiError, App};

impl App {
    /// Seeded and user categories at any depth, siblings ordered by sort order, name, id.
    /// Deleted and merged rows are left out; a live row whose parent is not live is a root,
    /// so nothing live is ever hidden.
    pub fn category_tree(
        &self,
        query: CategoryTreeQuery,
    ) -> Result<Vec<CategoryNodeDto>, ApiError> {
        command("category_tree", || {
            let kind = query.kind.map(wire);
            let mut rows: Vec<CategoryRow> = self
                .store
                .category_rows()?
                .into_iter()
                .filter(|r| !r.deleted && kind.as_ref().is_none_or(|k| *k == r.kind))
                .collect();
            rows.sort_by(|a, b| {
                (a.sort_order, &a.name, &a.id).cmp(&(b.sort_order, &b.name, &b.id))
            });
            let live: HashSet<String> = rows.iter().map(|r| r.id.clone()).collect();
            let mut children: BTreeMap<Option<String>, Vec<CategoryRow>> = BTreeMap::new();
            for r in rows {
                let parent = r.parent_id.clone().filter(|p| live.contains(p));
                children.entry(parent).or_default().push(r);
            }
            build(None, &mut children)
        })
    }
}

fn build(
    parent: Option<String>,
    children: &mut BTreeMap<Option<String>, Vec<CategoryRow>>,
) -> Result<Vec<CategoryNodeDto>, ApiError> {
    children
        .remove(&parent)
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            Ok(CategoryNodeDto {
                children: build(Some(r.id.clone()), children)?,
                kind: from_wire(&r.kind)?,
                sort_order: sort_order(r.sort_order)?,
                id: r.id,
                parent_id: parent.clone(),
                name: r.name,
                icon: r.icon,
                is_archived: r.is_archived,
            })
        })
        .collect()
}
