//! Spending by category (DEC-29 Q6): rolled-up expense actuals and their shares.

use std::collections::BTreeMap;

use mrk_money::{CurrencyCode, Money};

use super::flows::{Counts, flows};
use super::period::DateRange;
use super::{
    CategoryTree, ReportData, ReportError, Spending, SpendingRow, Viewer, add_to, excluded_list,
    sum_roots,
};
use crate::category::CategoryKind;

/// All of spending, in basis points.
const WHOLE_BP: u32 = 10_000;

/// Spending over the civil dates `from`–`to` (inclusive) for `viewer`: one row per expense
/// category, depth-first with siblings largest amount first (ties in tree order), with its
/// rolled-up net actual and its share of total spending in
/// basis points.
///
/// Shares are split top-down with the largest-remainder rule, so they are exact: the roots'
/// shares sum to 10 000, and a group's own share plus its children's equal its share. A
/// category whose amount is ≤ 0 gets 0. When total spending is ≤ 0 no share is meaningful
/// and every `share_bp` is `None`.
pub fn spending(
    data: &ReportData,
    viewer: &Viewer,
    from: &str,
    to: &str,
) -> Result<Spending, ReportError> {
    let range = DateRange::parse(from, to)?;
    let base = data.base;
    let tree = CategoryTree::new(&data.categories);
    let flows = flows(data, &tree, viewer, &range)?;

    let mut own = BTreeMap::new();
    for f in flows
        .categories
        .iter()
        .filter(|f| f.kind == CategoryKind::Expense)
    {
        add_to(&mut own, f.category, f.actual)?;
    }
    let rolled = tree.rollup(&own, base)?;
    let total = sum_roots(&tree, &rolled, CategoryKind::Expense, base)?;
    let shares = if total.minor > 0 {
        Some(shares(&tree, &own, &rolled, base)?)
    } else {
        None
    };

    let rows = tree
        .rows()
        .iter()
        .filter(|r| r.kind == CategoryKind::Expense)
        .map(|r| SpendingRow {
            category: r.clone(),
            amount: rolled[&r.id],
            share_bp: shares.as_ref().map(|s| s[&r.id]),
        })
        .collect();
    Ok(Spending {
        rows: largest_first(rows),
        total,
        excluded: excluded_list(
            flows.left_out_entries(|c| c == Counts::Category(CategoryKind::Expense)),
        ),
    })
}

/// Reorders tree-ordered rows so that siblings (and roots) come largest amount first (X-17),
/// still depth-first with each group's children right after it. The sort is stable, so ties
/// keep the tree's sibling order.
fn largest_first(rows: Vec<SpendingRow>) -> Vec<SpendingRow> {
    let mut children: BTreeMap<Option<String>, Vec<SpendingRow>> = BTreeMap::new();
    for r in rows {
        children
            .entry(r.category.parent_id.clone())
            .or_default()
            .push(r);
    }
    for list in children.values_mut() {
        list.sort_by_key(|r| std::cmp::Reverse(r.amount.minor));
    }
    let mut out = Vec::new();
    let mut stack: Vec<SpendingRow> = children.remove(&None).unwrap_or_default();
    stack.reverse();
    while let Some(r) = stack.pop() {
        if let Some(mut kids) = children.remove(&Some(r.category.id.clone())) {
            kids.reverse();
            stack.extend(kids);
        }
        out.push(r);
    }
    out
}

/// Splits 10 000 bp over the expense roots by amount, then each group's share over its own
/// amount and its children's, in tree (pre-)order so a parent is split before its children.
fn shares(
    tree: &CategoryTree,
    own: &BTreeMap<String, Money>,
    rolled: &BTreeMap<String, Money>,
    base: CurrencyCode,
) -> Result<BTreeMap<String, u32>, ReportError> {
    let weight = |m: Option<&Money>| m.map_or(0, |m| u64::try_from(m.minor).unwrap_or(0));
    let expense: Vec<_> = tree
        .rows()
        .iter()
        .filter(|r| r.kind == CategoryKind::Expense)
        .collect();
    let mut out = BTreeMap::new();

    let roots: Vec<_> = expense.iter().filter(|r| r.depth == 0).collect();
    let parts = split(
        WHOLE_BP,
        &roots
            .iter()
            .map(|r| weight(rolled.get(&r.id)))
            .collect::<Vec<_>>(),
        base,
    )?;
    for (r, p) in roots.iter().zip(parts) {
        out.insert(r.id.clone(), p);
    }
    for group in expense.iter().filter(|r| r.is_group) {
        let children: Vec<_> = expense
            .iter()
            .filter(|r| r.parent_id.as_deref() == Some(group.id.as_str()))
            .collect();
        let mut weights = vec![weight(own.get(&group.id))];
        weights.extend(children.iter().map(|c| weight(rolled.get(&c.id))));
        let parts = split(out[&group.id], &weights, base)?;
        for (c, p) in children.iter().zip(&parts[1..]) {
            out.insert(c.id.clone(), *p);
        }
    }
    Ok(out)
}

/// `bp` split by `weights` with `Money::allocate`'s largest-remainder rule (parts sum exactly to
/// `bp`); all zeros when every weight is zero.
fn split(bp: u32, weights: &[u64], base: CurrencyCode) -> Result<Vec<u32>, ReportError> {
    if weights.iter().all(|w| *w == 0) {
        return Ok(vec![0; weights.len()]);
    }
    Money::new(i64::from(bp), base)
        .allocate(weights)?
        .into_iter()
        .map(|m| u32::try_from(m.minor).map_err(|_| mrk_money::MoneyError::Overflow.into()))
        .collect()
}
