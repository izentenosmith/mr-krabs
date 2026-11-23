//! From posting rows to what reports add up (DEC-29 Q7/Q8): category actuals and saved
//! amounts in the base currency, for one viewer and one date range.

use std::collections::BTreeMap;

use mrk_money::Money;

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Flows<'a> {
    pub categories: Vec<CategoryFlow<'a>>,
    pub saved: Vec<SavedFlow>,
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

    let mut out = Flows::default();
    for legs in entries.values() {
        let in_view = |id: &str| accounts.get(id).is_some_and(|a| viewer.sees(&a.scope));
        let entry_in_view = legs
            .iter()
            .any(|p| matches!(&p.leg, Leg::Account(id) if in_view(id)));
        if !entry_in_view {
            continue;
        }
        let transfer = !legs.iter().any(|p| matches!(p.leg, Leg::Category(_)));
        if transfer {
            // Saved = the net of the in-view saving-account legs of a transfer: into savings
            // is +, out is −, savings ↔ savings nets to 0, a card payment has none.
            for p in legs {
                let Leg::Account(id) = &p.leg else { continue };
                let saving = accounts
                    .get(id.as_str())
                    .is_some_and(|a| a.kind.is_saving() && viewer.sees(&a.scope));
                if saving && p.amount.currency == data.base {
                    out.saved.push(SavedFlow {
                        month: Month::of_date(&p.entry_date),
                        amount: p.amount,
                    });
                }
            }
            continue;
        }
        for p in legs {
            let Leg::Category(id) = &p.leg else { continue };
            let Some(row) = tree.get(id) else { continue };
            if p.amount.currency != data.base {
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
