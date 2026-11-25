//! Category rules (FR-BU-01, FR-BU-07, FR-IM-07; docs/03 "Categories are data").
//!
//! Categories are user data: a tree of any depth per kind, edited, moved, merged and deleted
//! by the household. This module decides whether an edit is valid and what a merge changes;
//! `mrk-store` only writes the result.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use mrk_money::{Money, MoneyError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CategoryKind {
    Income,
    Expense,
}

impl CategoryKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Income => "income",
            Self::Expense => "expense",
        }
    }
}

/// The part of a category row the tree rules need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: CategoryKind,
    /// `false` for tombstoned rows (`deleted = 1`).
    pub live: bool,
    pub sort_order: i64,
    pub name: String,
}

/// Siblings' order, and the member a parent cycle is cut at (X-16): kind (income first),
/// sort order, accent-insensitive name, then id.
pub fn sibling_key(n: &CategoryNode) -> (CategoryKind, i64, String, &str) {
    (n.kind, n.sort_order, normalize_name(&n.name), n.id.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategoryError {
    Unknown(String),
    NotLive(String),
    ParentIsSelf,
    /// The new parent is the category itself or one of its descendants.
    Cycle,
    /// A child's kind must equal its parent's (schema v2 declares it too).
    ParentKindMismatch,
    /// A category's kind never changes; create a new one and merge instead.
    KindImmutable,
    MergeIntoSelf,
    MergeKindMismatch,
    /// Its children would be re-parented under one of their own descendants.
    MergeIntoDescendant,
    /// A move's position is past the end of the new parent's children.
    IndexOutOfRange {
        index: usize,
        len: usize,
    },
    Money(MoneyError),
}

impl fmt::Display for CategoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "unknown category {id}"),
            Self::NotLive(id) => write!(f, "category {id} is deleted"),
            Self::ParentIsSelf => write!(f, "a category cannot be its own parent"),
            Self::Cycle => write!(f, "the new parent is inside this category"),
            Self::ParentKindMismatch => write!(f, "a category's parent must be of the same kind"),
            Self::KindImmutable => write!(f, "a category's kind cannot change"),
            Self::MergeIntoSelf => write!(f, "a category cannot be merged into itself"),
            Self::MergeKindMismatch => write!(f, "categories of different kinds cannot be merged"),
            Self::MergeIntoDescendant => {
                write!(f, "a category cannot be merged into its own sub-category")
            }
            Self::IndexOutOfRange { index, len } => {
                write!(
                    f,
                    "position {index} is past the end of {len} sub-categories"
                )
            }
            Self::Money(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CategoryError {}

impl From<MoneyError> for CategoryError {
    fn from(e: MoneyError) -> Self {
        Self::Money(e)
    }
}

/// The key category names are compared and matched by (FR-IM-07): trimmed, inner whitespace
/// collapsed, lowercase, accents removed from vowels and `ç`. `ñ` stays a letter of its own,
/// so "Año" and "Ano" are different names. Both precomposed and combining-mark input give the
/// same key. Stored as `category.name_key`; import matching uses the same function.
pub fn normalize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut pending_space = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if is_combining_mark(c) {
            // A combining tilde after `n` is `ñ`; every other mark is an accent and dropped.
            if c == '\u{303}' && out.ends_with('n') {
                out.pop();
                out.push('ñ');
            }
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(fold_accent(c));
    }
    out
}

fn is_combining_mark(c: char) -> bool {
    ('\u{300}'..='\u{36f}').contains(&c)
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ý' | 'ÿ' => 'y',
        'ç' => 'c',
        other => other,
    }
}

fn index(nodes: &[CategoryNode]) -> BTreeMap<&str, &CategoryNode> {
    nodes.iter().map(|n| (n.id.as_str(), n)).collect()
}

fn live<'a>(
    by_id: &BTreeMap<&str, &'a CategoryNode>,
    id: &str,
) -> Result<&'a CategoryNode, CategoryError> {
    let n = by_id
        .get(id)
        .ok_or_else(|| CategoryError::Unknown(id.to_owned()))?;
    if n.live {
        Ok(n)
    } else {
        Err(CategoryError::NotLive(id.to_owned()))
    }
}

/// True when `candidate` is `ancestor` or sits anywhere below it.
fn is_within(by_id: &BTreeMap<&str, &CategoryNode>, candidate: &str, ancestor: &str) -> bool {
    let mut seen = BTreeSet::new();
    let mut cur = Some(candidate);
    while let Some(id) = cur {
        if id == ancestor {
            return true;
        }
        if !seen.insert(id) {
            return false; // already cyclic data: stop rather than loop
        }
        cur = by_id.get(id).and_then(|n| n.parent_id.as_deref());
    }
    false
}

/// Validates placing category `id` (of `kind`; it may not exist yet) under `new_parent`
/// (`None` = a root of its kind's tree). Used by create and move.
pub fn validate_parent(
    nodes: &[CategoryNode],
    id: &str,
    kind: CategoryKind,
    new_parent: Option<&str>,
) -> Result<(), CategoryError> {
    let Some(parent) = new_parent else {
        return Ok(());
    };
    if parent == id {
        return Err(CategoryError::ParentIsSelf);
    }
    let by_id = index(nodes);
    let p = live(&by_id, parent)?;
    if p.kind != kind {
        return Err(CategoryError::ParentKindMismatch);
    }
    if is_within(&by_id, parent, id) {
        return Err(CategoryError::Cycle);
    }
    Ok(())
}

/// A category's kind is fixed at creation (design A-03).
pub fn validate_kind_change(
    current: CategoryKind,
    requested: CategoryKind,
) -> Result<(), CategoryError> {
    if current == requested {
        Ok(())
    } else {
        Err(CategoryError::KindImmutable)
    }
}

/// The parent each live category is shown under (M1 #29 F3): its own parent when that is live,
/// otherwise none (a root). A parent cycle, which only sync can create, is cut at its
/// smallest member by [`sibling_key`], which becomes a root; so every live category appears
/// exactly once, whatever order the rows come in, and every walk up the result ends.
pub fn tree_parents(nodes: &[CategoryNode]) -> BTreeMap<String, Option<String>> {
    let by_id: BTreeMap<&str, &CategoryNode> = nodes
        .iter()
        .filter(|n| n.live)
        .map(|n| (n.id.as_str(), n))
        .collect();
    let mut parents: BTreeMap<String, Option<String>> = nodes
        .iter()
        .filter(|n| n.live)
        .map(|n| {
            let p = n
                .parent_id
                .clone()
                .filter(|p| by_id.contains_key(p.as_str()));
            (n.id.clone(), p)
        })
        .collect();
    // Nodes already known to reach a root.
    let mut rooted: BTreeSet<String> = BTreeSet::new();
    let ids: Vec<String> = parents.keys().cloned().collect();
    for start in ids {
        let mut path: Vec<String> = Vec::new();
        let mut cur = Some(start);
        while let Some(id) = cur {
            if rooted.contains(&id) {
                break;
            }
            if let Some(at) = path.iter().position(|p| *p == id) {
                // `path[at..]` is a cycle: its smallest member becomes a root.
                if let Some(min) = path[at..]
                    .iter()
                    .min_by_key(|m| sibling_key(by_id[m.as_str()]))
                {
                    parents.insert(min.clone(), None);
                }
                break;
            }
            cur = parents.get(&id).cloned().flatten();
            path.push(id);
        }
        rooted.extend(path);
    }
    parents
}

/// `id` followed by its ancestors, nearest first, in a `tree_parents` map (empty when `id` is
/// not in it).
pub fn lineage<'a>(parents: &'a BTreeMap<String, Option<String>>, id: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut cur = parents.get_key_value(id).map(|(k, _)| k.as_str());
    while let Some(c) = cur {
        out.push(c);
        // `tree_parents` is acyclic; the bound only guards a map built some other way.
        if out.len() > parents.len() {
            break;
        }
        cur = parents.get(c).and_then(|p| p.as_deref());
    }
    out
}

/// The new order of a parent's children after moving `id` to `index` among them.
/// `siblings` is their current order without `id`; `index` may equal its length (last).
pub fn move_position(
    siblings: &[String],
    id: &str,
    index: usize,
) -> Result<Vec<String>, CategoryError> {
    if index > siblings.len() {
        return Err(CategoryError::IndexOutOfRange {
            index,
            len: siblings.len(),
        });
    }
    let mut order: Vec<String> = siblings.iter().filter(|s| *s != id).cloned().collect();
    order.insert(index.min(order.len()), id.to_owned());
    Ok(order)
}

/// A live budget line of one of the two categories being merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetLineRow {
    pub id: String,
    pub category_id: String,
    pub period: String,
    pub scope: String,
    pub member_id: Option<String>,
    pub amount: Money,
}

/// A budget line of the merged category whose slot the target already fills: the target's
/// amount becomes `amount` and `absorbed_id` is deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetSum {
    pub target_id: String,
    pub amount: Money,
    pub absorbed_id: String,
}

/// Everything deleting `from` by merging it into `into` changes (FR-BU-07). Postings, payee
/// defaults, rules, recurring templates and staged rows are re-pointed wholesale by the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergePlan {
    pub from: String,
    pub into: String,
    /// Live children of `from`, re-parented under `into`.
    pub reparent: Vec<String>,
    /// Budget lines of `from` moved to `into` as they are.
    pub move_budget_lines: Vec<String>,
    /// Budget lines of `from` added to `into`'s line for the same period and scope.
    pub sum_budget_lines: Vec<BudgetSum>,
}

/// Plans merging `from` into `into`. Colliding budget lines are summed in Rust (Q-10);
/// a currency mismatch between them is an error, never a silent conversion.
pub fn plan_merge(
    nodes: &[CategoryNode],
    budget_lines: &[BudgetLineRow],
    from: &str,
    into: &str,
) -> Result<MergePlan, CategoryError> {
    if from == into {
        return Err(CategoryError::MergeIntoSelf);
    }
    let by_id = index(nodes);
    let f = live(&by_id, from)?;
    let t = live(&by_id, into)?;
    if f.kind != t.kind {
        return Err(CategoryError::MergeKindMismatch);
    }
    if is_within(&by_id, into, from) {
        return Err(CategoryError::MergeIntoDescendant);
    }
    let reparent = nodes
        .iter()
        .filter(|n| n.live && n.parent_id.as_deref() == Some(from))
        .map(|n| n.id.clone())
        .collect();

    let slot = |b: &BudgetLineRow| (b.period.clone(), b.scope.clone(), b.member_id.clone());
    let targets: BTreeMap<_, &BudgetLineRow> = budget_lines
        .iter()
        .filter(|b| b.category_id == into)
        .map(|b| (slot(b), b))
        .collect();
    let mut move_budget_lines = Vec::new();
    let mut sum_budget_lines = Vec::new();
    for b in budget_lines.iter().filter(|b| b.category_id == from) {
        match targets.get(&slot(b)) {
            Some(target) => sum_budget_lines.push(BudgetSum {
                target_id: target.id.clone(),
                amount: target.amount.checked_add(b.amount)?,
                absorbed_id: b.id.clone(),
            }),
            None => move_budget_lines.push(b.id.clone()),
        }
    }
    Ok(MergePlan {
        from: from.to_owned(),
        into: into.to_owned(),
        reparent,
        move_budget_lines,
        sum_budget_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use CategoryKind::{Expense, Income};
    use mrk_money::Currency;
    use proptest::prelude::*;

    fn node(id: &str, parent: Option<&str>, kind: CategoryKind) -> CategoryNode {
        CategoryNode {
            id: id.into(),
            parent_id: parent.map(Into::into),
            kind,
            live: true,
            sort_order: 0,
            name: id.into(),
        }
    }

    fn tree() -> Vec<CategoryNode> {
        vec![
            node("food", None, Expense),
            node("super", Some("food"), Expense),
            node("organic", Some("super"), Expense),
            node("delivery", Some("food"), Expense),
            node("salary", None, Income),
        ]
    }

    fn clp(v: i64) -> Money {
        Money::new(v, Currency::CLP.code)
    }

    fn line(id: &str, cat: &str, period: &str, amount: Money) -> BudgetLineRow {
        BudgetLineRow {
            id: id.into(),
            category_id: cat.into(),
            period: period.into(),
            scope: "household".into(),
            member_id: None,
            amount,
        }
    }

    #[test]
    fn normalizes_case_accents_and_spaces() {
        assert_eq!(normalize_name("  Supermercado "), "supermercado");
        assert_eq!(normalize_name("CAFÉ  y   Té"), "cafe y te");
        assert_eq!(normalize_name("Orgánico"), normalize_name("organico"));
        // Combining marks give the same key as precomposed letters.
        assert_eq!(normalize_name("Orga\u{301}nico"), "organico");
        assert_eq!(normalize_name("Ban\u{303}o"), normalize_name("Baño"));
        // ñ is a letter, not an accented n.
        assert_eq!(normalize_name("Año"), "año");
        assert_ne!(normalize_name("Año"), normalize_name("Ano"));
    }

    #[test]
    fn parent_must_exist_be_live_and_share_the_kind() {
        let t = tree();
        assert_eq!(validate_parent(&t, "new", Expense, Some("super")), Ok(()));
        assert_eq!(validate_parent(&t, "new", Expense, None), Ok(()));
        assert_eq!(
            validate_parent(&t, "new", Expense, Some("salary")),
            Err(CategoryError::ParentKindMismatch)
        );
        assert_eq!(
            validate_parent(&t, "new", Expense, Some("nope")),
            Err(CategoryError::Unknown("nope".into()))
        );
        let mut dead = tree();
        dead[3].live = false;
        assert_eq!(
            validate_parent(&dead, "new", Expense, Some("delivery")),
            Err(CategoryError::NotLive("delivery".into()))
        );
    }

    #[test]
    fn moves_that_would_create_a_cycle_are_rejected() {
        let t = tree();
        assert_eq!(
            validate_parent(&t, "food", Expense, Some("food")),
            Err(CategoryError::ParentIsSelf)
        );
        assert_eq!(
            validate_parent(&t, "food", Expense, Some("organic")),
            Err(CategoryError::Cycle)
        );
        assert_eq!(
            validate_parent(&t, "super", Expense, Some("delivery")),
            Ok(())
        );
    }

    #[test]
    fn kind_is_immutable() {
        assert_eq!(validate_kind_change(Expense, Expense), Ok(()));
        assert_eq!(
            validate_kind_change(Expense, Income),
            Err(CategoryError::KindImmutable)
        );
    }

    #[test]
    fn merge_reparents_children_and_sums_colliding_budgets() {
        let t = tree();
        let lines = [
            line("b1", "super", "2026-11", clp(100_000)),
            line("b2", "super", "2026-12", clp(90_000)),
            line("b3", "delivery", "2026-11", clp(20_000)),
        ];
        let plan = plan_merge(&t, &lines, "super", "delivery").unwrap();
        assert_eq!(plan.reparent, ["organic"]);
        assert_eq!(plan.move_budget_lines, ["b2"]);
        assert_eq!(
            plan.sum_budget_lines,
            [BudgetSum {
                target_id: "b3".into(),
                amount: clp(120_000),
                absorbed_id: "b1".into()
            }]
        );
    }

    #[test]
    fn merge_rejects_invalid_targets() {
        let t = tree();
        assert_eq!(
            plan_merge(&t, &[], "food", "food"),
            Err(CategoryError::MergeIntoSelf)
        );
        assert_eq!(
            plan_merge(&t, &[], "food", "salary"),
            Err(CategoryError::MergeKindMismatch)
        );
        assert_eq!(
            plan_merge(&t, &[], "food", "organic"),
            Err(CategoryError::MergeIntoDescendant)
        );
        let usd = Money::new(1, Currency::USD.code);
        let lines = [
            line("b1", "super", "2026-11", clp(1)),
            line("b2", "delivery", "2026-11", usd),
        ];
        assert!(matches!(
            plan_merge(&t, &lines, "super", "delivery"),
            Err(CategoryError::Money(_))
        ));
    }

    #[test]
    fn tree_parents_roots_orphans_and_breaks_cycles_at_the_smallest_id() {
        let mut t = tree();
        t[0].live = false; // food deleted: super and delivery become roots
        t.push(node("b", Some("a"), Expense));
        t.push(node("a", Some("c"), Expense));
        t.push(node("c", Some("b"), Expense));
        t.push(node("d", Some("c"), Expense));
        let p = tree_parents(&t);
        assert!(!p.contains_key("food"));
        assert_eq!(p["super"], None);
        assert_eq!(p["organic"].as_deref(), Some("super"));
        assert_eq!(p["a"], None);
        assert_eq!(p["b"].as_deref(), Some("a"));
        assert_eq!(p["c"].as_deref(), Some("b"));
        assert_eq!(lineage(&p, "d"), ["d", "c", "b", "a"]);
        assert_eq!(lineage(&p, "organic"), ["organic", "super"]);
        assert!(lineage(&p, "food").is_empty());
    }

    #[test]
    fn a_cycle_is_cut_by_kind_sort_order_name_then_id() {
        let at = |id: &str, parent: &str, sort: i64, name: &str| CategoryNode {
            sort_order: sort,
            name: name.into(),
            ..node(id, Some(parent), Expense)
        };
        // Sort order beats id.
        let p = tree_parents(&[at("a", "b", 5, "A"), at("b", "a", 1, "B")]);
        assert_eq!((p["a"].as_deref(), p["b"].as_deref()), (Some("b"), None));
        // Then the accent-insensitive name: "álamo" < "Zeta" although 'Z' < 'á'.
        let p = tree_parents(&[at("a", "b", 0, "Zeta"), at("b", "a", 0, "álamo")]);
        assert_eq!(p["b"], None);
        // Then the id.
        let p = tree_parents(&[at("b", "a", 0, "X"), at("a", "b", 0, "x")]);
        assert_eq!(p["a"], None);
    }

    #[test]
    fn move_position_inserts_at_the_index() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            move_position(&s(&["a", "b"]), "x", 0).unwrap(),
            s(&["x", "a", "b"])
        );
        assert_eq!(
            move_position(&s(&["a", "b"]), "x", 2).unwrap(),
            s(&["a", "b", "x"])
        );
        assert_eq!(
            move_position(&s(&["a", "b"]), "x", 3),
            Err(CategoryError::IndexOutOfRange { index: 3, len: 2 })
        );
    }

    proptest! {
        /// Whatever the parent links (cycles included), every live node is in
        /// `tree_parents` and every walk up it reaches a root.
        #[test]
        fn tree_parents_is_acyclic_and_complete(
            links in proptest::collection::vec(proptest::option::of(0usize..8), 8),
            dead in proptest::collection::vec(any::<bool>(), 8),
        ) {
            let nodes: Vec<CategoryNode> = links
                .iter()
                .enumerate()
                .map(|(i, p)| CategoryNode {
                    id: format!("c{i}"),
                    parent_id: p.filter(|p| *p != i).map(|p| format!("c{p}")),
                    kind: Expense,
                    live: !dead[i],
                    sort_order: 0,
                    name: format!("c{i}"),
                })
                .collect();
            let parents = tree_parents(&nodes);
            prop_assert_eq!(parents.len(), dead.iter().filter(|d| !**d).count());
            for id in parents.keys() {
                let up = lineage(&parents, id);
                prop_assert!(up.len() <= parents.len());
                let top = up.last().copied().unwrap_or_default();
                prop_assert_eq!(&parents[top], &None);
            }
        }

        /// Applying only the moves `validate_parent` accepts never produces a cycle.
        #[test]
        fn accepted_moves_never_create_a_cycle(
            moves in proptest::collection::vec((0usize..8, proptest::option::of(0usize..8)), 0..60)
        ) {
            let mut nodes: Vec<CategoryNode> =
                (0..8).map(|i| node(&format!("c{i}"), None, Expense)).collect();
            for (child, parent) in moves {
                let id = format!("c{child}");
                let parent_id = parent.map(|p| format!("c{p}"));
                if validate_parent(&nodes, &id, Expense, parent_id.as_deref()).is_ok() {
                    nodes[child].parent_id = parent_id;
                }
            }
            let by_id = index(&nodes);
            for n in &nodes {
                let mut cur = n.parent_id.as_deref();
                let mut steps = 0;
                while let Some(p) = cur {
                    prop_assert_ne!(p, n.id.as_str());
                    steps += 1;
                    prop_assert!(steps <= nodes.len());
                    cur = by_id[p].parent_id.as_deref();
                }
            }
        }

        /// Normalizing is idempotent: a stored key normalizes to itself.
        #[test]
        fn normalize_is_idempotent(s in "\\PC{0,24}") {
            let once = normalize_name(&s);
            prop_assert_eq!(normalize_name(&once), once.clone());
        }
    }
}
