//! Categories (FR-BU-01, FR-BU-07; DEC-30 Q10): the live tree (`category_tree`, REQ-13) and
//! the household's edits to it — create, update, move, archive and delete-with-merge
//! (REQ-15–REQ-19). The tree rules live in `mrk_domain::category`; every write is one SQL
//! transaction logged through the change log (ADR-0010). Seeded rows are edited like any
//! other (ADR-0008).

use std::collections::{BTreeMap, HashMap};

use mrk_domain::{
    BudgetLineRow, CategoryError, CategoryNode, lineage, move_position, normalize_name, plan_merge,
    tree_parents, validate_parent,
};
use mrk_money::Money;
use mrk_store::{CategoryMergeWrite, CategoryRow};

use crate::api::{CategoryKind, CategoryNodeDto, CategoryTreeQuery};
use crate::support::{
    Fields, check_id, command, currency_code, field_error, from_wire, new_id, sort_order, wire,
};
use crate::{ApiError, App, ErrorCode};

/// Longest category name, in characters (DEC-30 Q10).
pub(crate) const CATEGORY_NAME_MAX: usize = 80;
/// Longest icon name, in characters.
const ICON_MAX: usize = 64;

impl App {
    /// Seeded and user categories at any depth, siblings ordered by sort order, name, id.
    /// Deleted and merged rows are left out; a live row whose parent is not live is a root,
    /// and a parent cycle (sync only) is broken at its smallest id, so every live row appears
    /// exactly once (M1 #29 F3). `isArchived` is the archive flag in effect: a category is
    /// archived when it or any ancestor is (REQ-18).
    pub fn category_tree(
        &self,
        query: CategoryTreeQuery,
    ) -> Result<Vec<CategoryNodeDto>, ApiError> {
        command("category_tree", || {
            let kind = query.kind.map(wire);
            let all = self.store.category_rows()?;
            let parents = tree_parents(&nodes_of(&all)?);
            let mut rows: Vec<CategoryRow> = all
                .into_iter()
                .filter(|r| !r.deleted && kind.as_ref().is_none_or(|k| *k == r.kind))
                .collect();
            rows.sort_by(tree_order);
            let mut children: BTreeMap<Option<String>, Vec<CategoryRow>> = BTreeMap::new();
            for r in rows {
                let parent = parents.get(&r.id).cloned().flatten();
                children.entry(parent).or_default().push(r);
            }
            build(None, false, &mut children)
        })
    }

    /// Creates a category of `kind` under `parent_id` (`None`: a root of its kind's tree),
    /// placed after its siblings (REQ-15).
    pub fn category_create(
        &mut self,
        kind: CategoryKind,
        parent_id: Option<&str>,
        name: &str,
        icon: Option<&str>,
    ) -> Result<CategoryNodeDto, ApiError> {
        command("category_create", || {
            let mut f = Fields::default();
            let name = f.name("name", name, CATEGORY_NAME_MAX);
            let icon = f.optional("icon", icon, ICON_MAX);
            f.finish()?;
            let rows = self.store.category_rows()?;
            let id = new_id();
            let kind_code = wire(kind);
            check_parent(&rows, &id, &kind_code, parent_id)?;
            let mut row = CategoryRow {
                id,
                parent_id: parent_id.map(str::to_owned),
                kind: kind_code,
                name_key: normalize_name(&name),
                name,
                icon,
                is_archived: false,
                sort_order: 0,
                deleted: false,
            };
            check_unique_name(&rows, &row)?;
            row.sort_order = live_siblings(&rows, &row)
                .map(|s| s.sort_order.saturating_add(1))
                .max()
                .unwrap_or(0);
            self.store.write(|w| w.insert_category(&row))?;
            tracing::debug!(category = %row.id, "category created");
            self.category_dto(&row.id)
        })
    }

    /// Renames a category and/or changes its icon (REQ-16). `None` keeps a field; a blank icon
    /// clears it. Its kind never changes. Past entries show the new name: they reference the
    /// category, not its name.
    pub fn category_update(
        &mut self,
        id: &str,
        name: Option<&str>,
        icon: Option<&str>,
    ) -> Result<CategoryNodeDto, ApiError> {
        command("category_update", || {
            let mut f = Fields::default();
            let name = name.map(|n| f.name("name", n, CATEGORY_NAME_MAX));
            let icon = icon.map(|i| f.optional("icon", Some(i), ICON_MAX));
            f.finish()?;
            let rows = self.store.category_rows()?;
            let mut row = live(&rows, id)?.clone();
            if let Some(name) = name {
                row.name_key = normalize_name(&name);
                row.name = name;
                check_unique_name(&rows, &row)?;
            }
            if let Some(icon) = icon {
                row.icon = icon;
            }
            self.store.write(|w| w.update_category(&row))?;
            self.category_dto(id)
        })
    }

    /// Moves a category under `parent_id` (`None`: a root) at position `index` among that
    /// parent's live children of its kind, in tree order (REQ-17). Those children are
    /// renumbered `0..n`; only rows whose parent or position changes are written.
    pub fn category_move(
        &mut self,
        id: &str,
        parent_id: Option<&str>,
        index: usize,
    ) -> Result<CategoryNodeDto, ApiError> {
        command("category_move", || {
            let rows = self.store.category_rows()?;
            let mut row = live(&rows, id)?.clone();
            check_parent(&rows, id, &row.kind, parent_id)?;
            row.parent_id = parent_id.map(str::to_owned);
            check_unique_name(&rows, &row)?;
            let mut siblings: Vec<&CategoryRow> = live_siblings(&rows, &row).collect();
            siblings.sort_by(|a, b| tree_order(a, b));
            let ids: Vec<String> = siblings.iter().map(|s| s.id.clone()).collect();
            let order = move_position(&ids, id, index)
                .map_err(|e| field_error(ErrorCode::Validation, "index", "outOfRange", e))?;
            let by_id: HashMap<&str, &CategoryRow> =
                siblings.iter().map(|s| (s.id.as_str(), *s)).collect();
            self.store.write(|w| {
                for (pos, sid) in order.iter().enumerate() {
                    let pos = i64::try_from(pos).map_err(ApiError::internal)?;
                    if sid == id {
                        row.sort_order = pos;
                        w.update_category(&row)?;
                    } else if let Some(s) = by_id.get(sid.as_str()).filter(|s| s.sort_order != pos)
                    {
                        w.update_category(&CategoryRow {
                            sort_order: pos,
                            ..(*s).clone()
                        })?;
                    }
                }
                Ok::<_, ApiError>(())
            })?;
            self.category_dto(id)
        })
    }

    /// Archives or unarchives one category (REQ-18). Archiving hides it and its whole subtree
    /// from pickers (the flag in effect, see [`App::category_tree`]); history and reports keep
    /// it, and existing entries in it stay valid. Unarchiving restores the subtree, except
    /// sub-categories archived on their own. One row written, none when nothing changes.
    pub fn category_archive(
        &mut self,
        id: &str,
        archived: bool,
    ) -> Result<CategoryNodeDto, ApiError> {
        command("category_archive", || {
            let rows = self.store.category_rows()?;
            let mut row = live(&rows, id)?.clone();
            if row.is_archived != archived {
                row.is_archived = archived;
                self.store.write(|w| w.update_category(&row))?;
            }
            self.category_dto(id)
        })
    }

    /// Deletes a category (FR-BU-07, REQ-19). With `merge_into`, everything that references it
    /// moves there first (postings, budget lines with colliding slots summed, sub-categories,
    /// payee defaults, rules, recurring templates, staged rows). Without it, the category must
    /// be unused. Every row it touches is stamped by the local clock and logged (ADR-0010).
    pub fn category_delete(&mut self, id: &str, merge_into: Option<&str>) -> Result<(), ApiError> {
        command("category_delete", || {
            let rows = self.store.category_rows()?;
            live(&rows, id)?;
            let Some(into) = merge_into else {
                if self.store.category_reference_count(id)? > 0 {
                    return Err(field_error(
                        ErrorCode::Validation,
                        "mergeIntoId",
                        "required",
                        "the category has transactions, budgets or sub-categories: choose a category to merge it into",
                    ));
                }
                return Ok(self.store.delete_category(id)?);
            };
            check_id(into)?;
            let lines = self
                .store
                .live_budget_lines(&[id, into])?
                .into_iter()
                .map(|b| {
                    Ok(BudgetLineRow {
                        amount: Money::new(b.amount_minor, currency_code(&b.currency_code)?),
                        id: b.id,
                        category_id: b.category_id,
                        period: b.period,
                        scope: b.scope,
                        member_id: b.member_id,
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            let plan = plan_merge(&nodes_of(&rows)?, &lines, id, into)
                .map_err(|e| category_error("mergeIntoId", e))?;
            let write = CategoryMergeWrite {
                from: plan.from,
                into: plan.into,
                reparent: plan.reparent,
                move_budget_lines: plan.move_budget_lines,
                sum_budget_lines: plan
                    .sum_budget_lines
                    .into_iter()
                    .map(|s| (s.target_id, s.amount.minor, s.absorbed_id))
                    .collect(),
            };
            Ok(self.store.apply_category_merge(&write)?)
        })
    }

    /// Whether `id` is archived in effect: it or one of its ancestors is (REQ-18).
    pub(crate) fn category_archived_in_effect(&self, id: &str) -> Result<bool, ApiError> {
        let rows = self.store.category_rows()?;
        Ok(archived_in_effect(
            &rows,
            &tree_parents(&nodes_of(&rows)?),
            id,
        ))
    }

    /// A live category as the commands return it (no children).
    fn category_dto(&self, id: &str) -> Result<CategoryNodeDto, ApiError> {
        let rows = self.store.category_rows()?;
        let parents = tree_parents(&nodes_of(&rows)?);
        let row = live(&rows, id)?;
        Ok(CategoryNodeDto {
            id: row.id.clone(),
            parent_id: parents.get(id).cloned().flatten(),
            kind: from_wire(&row.kind)?,
            name: row.name.clone(),
            icon: row.icon.clone(),
            is_archived: archived_in_effect(&rows, &parents, id),
            sort_order: sort_order(row.sort_order)?,
            children: Vec::new(),
        })
    }
}

/// Category rows as the domain's tree nodes.
pub(crate) fn nodes_of(rows: &[CategoryRow]) -> Result<Vec<CategoryNode>, ApiError> {
    rows.iter()
        .map(|r| {
            Ok(CategoryNode {
                id: r.id.clone(),
                parent_id: r.parent_id.clone(),
                kind: match r.kind.as_str() {
                    "income" => mrk_domain::CategoryKind::Income,
                    "expense" => mrk_domain::CategoryKind::Expense,
                    other => {
                        return Err(ApiError::internal(format!(
                            "unknown category kind {other:?}"
                        )));
                    }
                },
                live: !r.deleted,
            })
        })
        .collect()
}

fn archived_in_effect(
    rows: &[CategoryRow],
    parents: &BTreeMap<String, Option<String>>,
    id: &str,
) -> bool {
    let archived: HashMap<&str, bool> = rows
        .iter()
        .map(|r| (r.id.as_str(), r.is_archived))
        .collect();
    lineage(parents, id)
        .into_iter()
        .any(|c| archived.get(c).copied().unwrap_or(false))
}

/// Siblings in tree order: sort order, name, id.
fn tree_order(a: &CategoryRow, b: &CategoryRow) -> std::cmp::Ordering {
    (a.sort_order, &a.name, &a.id).cmp(&(b.sort_order, &b.name, &b.id))
}

/// A live (not deleted) category, or `NOT_FOUND` (any id, as M1's `category_delete`).
fn live<'a>(rows: &'a [CategoryRow], id: &str) -> Result<&'a CategoryRow, ApiError> {
    rows.iter()
        .find(|r| r.id == id && !r.deleted)
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, format!("no category {id}")))
}

/// The other live categories with `row`'s kind and parent.
fn live_siblings<'a>(
    rows: &'a [CategoryRow],
    row: &CategoryRow,
) -> impl Iterator<Item = &'a CategoryRow> + use<'a> {
    let (id, kind, parent) = (row.id.clone(), row.kind.clone(), row.parent_id.clone());
    rows.iter()
        .filter(move |r| !r.deleted && r.id != id && r.kind == kind && r.parent_id == parent)
}

/// A live sibling with the same normalized name → `errors.field.duplicate` on `name`
/// (the rule `ux_category_sibling_name` stores).
fn check_unique_name(rows: &[CategoryRow], row: &CategoryRow) -> Result<(), ApiError> {
    if live_siblings(rows, row).any(|s| s.name_key == row.name_key) {
        return Err(field_error(
            ErrorCode::Validation,
            "name",
            "duplicate",
            "a category with this name is already there",
        ));
    }
    Ok(())
}

/// `parent` may hold category `id` of `kind`: live, same kind, not `id` or below it, and not
/// archived in effect (a new or moved category never lands hidden from pickers).
fn check_parent(
    rows: &[CategoryRow],
    id: &str,
    kind: &str,
    parent: Option<&str>,
) -> Result<(), ApiError> {
    let Some(parent) = parent else {
        return Ok(());
    };
    check_id(parent)
        .map_err(|e| field_error(ErrorCode::Validation, "parentId", "invalid", e.message))?;
    let nodes = nodes_of(rows)?;
    let kind = match kind {
        "income" => mrk_domain::CategoryKind::Income,
        _ => mrk_domain::CategoryKind::Expense,
    };
    validate_parent(&nodes, id, kind, Some(parent)).map_err(|e| category_error("parentId", e))?;
    if archived_in_effect(rows, &tree_parents(&nodes), parent) {
        return Err(field_error(
            ErrorCode::Validation,
            "parentId",
            "archived",
            "the parent category is archived",
        ));
    }
    Ok(())
}

/// A tree rule's refusal as a field error on `field`: unknown or deleted → `NOT_FOUND`,
/// everything else → `VALIDATION`.
fn category_error(field: &str, e: CategoryError) -> ApiError {
    let (code, reason) = match &e {
        CategoryError::Unknown(_) | CategoryError::NotLive(_) => (ErrorCode::NotFound, "notFound"),
        CategoryError::ParentKindMismatch | CategoryError::MergeKindMismatch => {
            (ErrorCode::Validation, "wrongKind")
        }
        CategoryError::ParentIsSelf
        | CategoryError::Cycle
        | CategoryError::MergeIntoSelf
        | CategoryError::MergeIntoDescendant => (ErrorCode::Validation, "cycle"),
        _ => (ErrorCode::Validation, "invalid"),
    };
    field_error(code, field, reason, e)
}

fn build(
    parent: Option<String>,
    parent_archived: bool,
    children: &mut BTreeMap<Option<String>, Vec<CategoryRow>>,
) -> Result<Vec<CategoryNodeDto>, ApiError> {
    children
        .remove(&parent)
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let archived = parent_archived || r.is_archived;
            Ok(CategoryNodeDto {
                children: build(Some(r.id.clone()), archived, children)?,
                kind: from_wire(&r.kind)?,
                sort_order: sort_order(r.sort_order)?,
                id: r.id,
                parent_id: parent.clone(),
                name: r.name,
                icon: r.icon,
                is_archived: archived,
            })
        })
        .collect()
}
