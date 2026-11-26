//! From posting rows to what reports add up (DEC-29 Q7/Q8): category actuals and saved
//! amounts in the base currency, for one viewer and one date range.

use std::collections::{BTreeMap, BTreeSet};

use mrk_money::{CurrencyCode, Money};

use super::period::{DateRange, Month, day_of};
use super::{CategoryTree, ReportAccount, ReportData, ReportError, ReportPosting, Viewer};
use crate::category::CategoryKind;
use crate::date::validate_civil_date;
use crate::ledger::Leg;

/// What one category leg adds to its category's actual: expense legs as they are (a refund
/// is negative), income legs negated so income reads positive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CategoryFlow<'a> {
    pub month: Month,
    pub day: u32,
    pub category: &'a str,
    pub kind: CategoryKind,
    pub actual: Money,
}

/// Money moved by a transfer into (+) or out of (−) a savings or investment account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SavedFlow {
    pub month: Month,
    pub amount: Money,
}

/// What a left-out leg would have added to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Counts {
    Category(CategoryKind),
    Saved,
}

/// A leg that would count but is not in the base currency (DEC-29 Q8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LeftOut<'a> {
    pub entry_id: &'a str,
    pub currency: CurrencyCode,
    pub counts: Counts,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Flows<'a> {
    pub categories: Vec<CategoryFlow<'a>>,
    pub saved: Vec<SavedFlow>,
    pub left_out: Vec<LeftOut<'a>>,
}

impl Flows<'_> {
    /// Entries left out per currency, among the legs `relevant` keeps; each entry once.
    pub fn left_out_entries(
        &self,
        relevant: impl Fn(Counts) -> bool,
    ) -> BTreeMap<CurrencyCode, u32> {
        let mut entries: BTreeMap<CurrencyCode, BTreeSet<&str>> = BTreeMap::new();
        for l in self.left_out.iter().filter(|l| relevant(l.counts)) {
            entries.entry(l.currency).or_default().insert(l.entry_id);
        }
        entries
            .into_iter()
            .map(|(c, ids)| (c, u32::try_from(ids.len()).unwrap_or(u32::MAX)))
            .collect()
    }
}

/// Reads the postings of `data` that fall in `range` and in the viewer's view.
///
/// An entry's category legs count when one of its account legs is in view; a transfer (no
/// category leg) is never income or expense and only moves "saved". Opening-balance entries
/// never count (M1/DEC-4, ADR-0011).
pub(crate) fn flows<'a>(
    data: &'a ReportData,
    tree: &CategoryTree,
    viewer: &Viewer,
    range: &DateRange,
) -> Result<Flows<'a>, ReportError> {
    let accounts: BTreeMap<&str, &ReportAccount> =
        data.accounts.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut entries: BTreeMap<&str, Vec<&ReportPosting>> = BTreeMap::new();
    for p in &data.postings {
        if !p.opening_balance
            && validate_civil_date(&p.entry_date).is_ok()
            && range.contains(&p.entry_date)
        {
            entries.entry(p.entry_id.as_str()).or_default().push(p);
        }
    }

    // An account leg's account, when it is known and in the viewer's view.
    let account_in_view = |id: &str| accounts.get(id).copied().filter(|a| viewer.sees(&a.scope));

    let mut out = Flows::default();
    for legs in entries.values() {
        let entry_in_view = legs
            .iter()
            .any(|p| matches!(&p.leg, Leg::Account(id) if account_in_view(id).is_some()));
        if !entry_in_view {
            continue;
        }
        let transfer = !legs.iter().any(|p| matches!(p.leg, Leg::Category(_)));
        if transfer {
            // Saved = the net of the in-view saving-account legs of a transfer: into savings
            // is +, out is −, savings ↔ savings nets to 0, a card payment has none. A saving
            // leg in another currency leaves the transfer's saved out as a whole (never
            // half-counted, F2) and its entry is counted in `excluded`.
            let is_saving = |p: &&&ReportPosting| match &p.leg {
                Leg::Account(id) => account_in_view(id).is_some_and(|a| a.kind.is_saving()),
                _ => false,
            };
            let saving: Vec<&&ReportPosting> = legs.iter().filter(is_saving).collect();
            if let Some(p) = saving.iter().find(|p| p.amount.currency != data.base) {
                out.left_out.push(LeftOut {
                    entry_id: &p.entry_id,
                    currency: p.amount.currency,
                    counts: Counts::Saved,
                });
            } else {
                out.saved.extend(saving.iter().map(|p| SavedFlow {
                    month: Month::of_date(&p.entry_date),
                    amount: p.amount,
                }));
            }
            continue;
        }
        for p in legs {
            let Leg::Category(id) = &p.leg else { continue };
            let Some(row) = tree.get(id) else { continue };
            if p.amount.currency != data.base {
                out.left_out.push(LeftOut {
                    entry_id: &p.entry_id,
                    currency: p.amount.currency,
                    counts: Counts::Category(row.kind),
                });
                continue;
            }
            let actual = match row.kind {
                CategoryKind::Expense => p.amount,
                CategoryKind::Income => p.amount.checked_neg()?,
            };
            out.categories.push(CategoryFlow {
                month: Month::of_date(&p.entry_date),
                day: day_of(&p.entry_date),
                category: id,
                kind: row.kind,
                actual,
            });
        }
    }
    Ok(out)
}
