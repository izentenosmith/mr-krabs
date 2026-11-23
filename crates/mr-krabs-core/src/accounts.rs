//! Accounts (REQ-2): create, update, archive, get and list with balances derived from the
//! ledger in Rust (ADR-0003, design Q8). An account's opening balance is one
//! `opening_balance` entry (DEC-4, ADR-0011), kept for the account's whole life: removing it
//! soft-deletes the entry, adding it back restores the same entry and posting ids.

use std::collections::BTreeMap;

use mrk_domain::{
    AccountInfo, BalanceRow, DomainError, LedgerContext, LegStatus, NewTx, Posting, StoredLeg,
    account_balances, ensure_unlocked, plan_update, validate_civil_date,
};
use mrk_money::Money;
use mrk_store::{AccountRow, EntryRow, PostingRow, Write};

use crate::api::{
    AccountDetailDto, AccountDto, AccountInput, AccountListQuery, AccountScope, AccountSummaryDto,
    OpeningBalanceDto, View, Visibility,
};
use crate::household::NAME_MAX;
use crate::support::{
    Fields, check_id, command, currency_code, domain_error, field_error, from_wire, new_id,
    parse_amount, posting_row, sort_order, stored_leg, wire,
};
use crate::{ApiError, App, ErrorCode, MoneyDto};

const INSTITUTION_MAX: usize = 100;
const NUMBER_MASK_MAX: usize = 20;
/// A mask shows at most the last four digits (of any script) of a number, never the number.
const NUMBER_MASK_DIGITS: usize = 4;
/// Balances count every live leg, at any date (Q-household-accounts-2).
const ALL_DATES: &str = "9999-12-31";
/// `journal_entry.kind` of an account's opening-balance entry (DEC-4).
pub(crate) const OPENING_BALANCE: &str = "opening_balance";

/// An opening balance as the user set it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Opening {
    amount: Money,
    date: String,
}

/// An account's opening-balance entry as stored, live or deleted.
struct StoredOpening {
    entry: EntryRow,
    postings: Vec<PostingRow>,
}

impl StoredOpening {
    /// The stored postings as domain legs.
    fn legs(&self) -> Result<Vec<StoredLeg>, ApiError> {
        self.postings.iter().map(stored_leg).collect()
    }

    fn live(&self) -> Option<Opening> {
        if self.entry.deleted {
            return None;
        }
        let leg = self.postings.iter().find(|p| p.leg == "account")?;
        Some(Opening {
            amount: Money::new(leg.amount_minor, currency_code(&leg.currency_code).ok()?),
            date: self.entry.entry_date.clone(),
        })
    }
}

impl App {
    pub fn account_create(&mut self, input: AccountInput) -> Result<AccountDto, ApiError> {
        command("account_create", || {
            let me = self.me()?;
            let (row, opening) = account_from_input(None, &input, &me)?;
            self.store.write(|w| {
                w.insert_account(&row)?;
                if let Some(o) = &opening {
                    insert_opening(w, &row, o)?;
                }
                Ok::<_, ApiError>(())
            })?;
            tracing::debug!(account = %row.id, "account created");
            account_dto(&row)
        })
    }

    pub fn account_update(
        &mut self,
        id: String,
        input: AccountInput,
    ) -> Result<AccountDto, ApiError> {
        command("account_update", || {
            let me = self.me()?;
            let existing = self.live_account(&id)?;
            let (row, opening) = account_from_input(Some(&existing), &input, &me)?;
            let stored = self.stored_opening(&id)?;
            let currency_changed = row.currency_code != existing.currency_code;
            // Unchanged = same amount (currency included) and date, however it was typed (X-8).
            let opening_changed = opening != stored.as_ref().and_then(StoredOpening::live);
            if (currency_changed || opening_changed)
                && self.store.account_has_postings_outside(
                    &id,
                    stored.as_ref().map(|s| s.entry.id.as_str()),
                )?
            {
                return Err(if currency_changed {
                    field_error(
                        ErrorCode::CurrencyMismatch,
                        "currency",
                        "lockedByPostings",
                        "the account has transactions in its currency",
                    )
                } else {
                    field_error(
                        ErrorCode::Validation,
                        "openingBalance.amount",
                        "lockedByPostings",
                        "the account has transactions: its opening balance is fixed",
                    )
                });
            }
            self.store.write(|w| {
                if currency_changed {
                    // The account's (id, currency) is the postings' foreign key: both move.
                    w.defer_foreign_keys()?;
                }
                w.update_account(&row)?;
                match (stored, &opening) {
                    (None, None) => {}
                    (None, Some(o)) => insert_opening(w, &row, o)?,
                    (Some(s), Some(o)) => rewrite_opening(w, &row, s, o)?,
                    (Some(s), None) => remove_opening(w, &row, s)?,
                }
                Ok::<_, ApiError>(())
            })?;
            account_dto(&row)
        })
    }

    pub fn account_archive(&mut self, id: String) -> Result<AccountDto, ApiError> {
        command("account_archive", || {
            let mut row = self.live_account(&id)?;
            if !row.is_archived {
                row.is_archived = true;
                self.store.write(|w| w.update_account(&row))?;
            }
            account_dto(&row)
        })
    }

    pub fn account_get(&self, id: String) -> Result<AccountDetailDto, ApiError> {
        command("account_get", || {
            let row = self.live_account(&id)?;
            let opening =
                self.stored_opening(&id)?
                    .and_then(|s| s.live())
                    .map(|o| OpeningBalanceDto {
                        amount: MoneyDto::from_money(o.amount),
                        date: o.date,
                    });
            Ok(AccountDetailDto {
                balance: self.balance_of(&row, &self.balances()?)?,
                account: account_dto(&row)?,
                opening_balance: opening,
            })
        })
    }

    pub fn account_list(
        &self,
        query: AccountListQuery,
    ) -> Result<Vec<AccountSummaryDto>, ApiError> {
        command("account_list", || {
            let include_archived = query.include_archived.unwrap_or(false);
            let me = self.store.local_member_id()?;
            let balances = self.balances()?;
            self.store
                .accounts()?
                .into_iter()
                .filter(|a| include_archived || !a.is_archived)
                .filter(|a| match query.view {
                    View::Combined => true,
                    View::Household => a.scope == "household",
                    View::Personal => {
                        a.scope == "personal"
                            && a.owner_member_id.is_some()
                            && a.owner_member_id == me
                    }
                })
                .map(|a| {
                    Ok(AccountSummaryDto {
                        balance: self.balance_of(&a, &balances)?,
                        account: account_dto(&a)?,
                    })
                })
                .collect()
        })
    }

    fn live_account(&self, id: &str) -> Result<AccountRow, ApiError> {
        check_id(id)?;
        self.store
            .account(id)?
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, format!("no account {id}")))
    }

    fn stored_opening(&self, account_id: &str) -> Result<Option<StoredOpening>, ApiError> {
        let Some(entry) = self.store.opening_balance_entry(account_id)? else {
            return Ok(None);
        };
        let postings = self.store.entry_postings(&entry.id)?;
        Ok(Some(StoredOpening { entry, postings }))
    }

    /// Balance of every account with a counted leg (no SQL aggregation, rule 3).
    fn balances(&self) -> Result<BTreeMap<String, Money>, ApiError> {
        let rows = self
            .store
            .account_legs()?
            .into_iter()
            .map(|l| {
                Ok(BalanceRow {
                    posting: Posting::account(
                        &l.account_id,
                        Money::new(l.amount_minor, currency_code(&l.currency_code)?),
                    ),
                    entry_date: l.entry_date,
                    live: l.live,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        account_balances(&rows, ALL_DATES).map_err(domain_error)
    }

    fn balance_of(
        &self,
        a: &AccountRow,
        all: &BTreeMap<String, Money>,
    ) -> Result<MoneyDto, ApiError> {
        let zero = Money::zero(currency_code(&a.currency_code)?);
        Ok(MoneyDto::from_money(
            all.get(&a.id).copied().unwrap_or(zero),
        ))
    }
}

/// Checks an input and builds the row to store (keeping an existing account's id, archive
/// flag and sort order) and the opening balance it asks for (`None` for none or zero).
fn account_from_input(
    existing: Option<&AccountRow>,
    input: &AccountInput,
    me: &str,
) -> Result<(AccountRow, Option<Opening>), ApiError> {
    let mut f = Fields::default();
    let name = f.name("name", &input.name, NAME_MAX);
    let currency = f.currency("currency", &input.currency);
    let institution = f.optional("institution", input.institution.as_deref(), INSTITUTION_MAX);
    let number_mask = f.optional("numberMask", input.number_mask.as_deref(), NUMBER_MASK_MAX);
    if let Some(mask) = &number_mask {
        if mask.chars().filter(|c| c.is_numeric()).count() > NUMBER_MASK_DIGITS {
            f.add("numberMask", "fullNumber");
        }
    }
    let scope = input.scope.unwrap_or(AccountScope::Household);
    let visibility = match scope {
        AccountScope::Household => {
            if input
                .visibility
                .is_some_and(|v| v != Visibility::SharedFull)
            {
                f.add("visibility", "notAllowed");
            }
            Visibility::SharedFull
        }
        AccountScope::Personal => input.visibility.unwrap_or(Visibility::SharedSummary),
    };
    let opening_date = input.opening_balance.as_ref().map(|o| o.date.clone());
    f.finish()?;
    let currency = currency.ok_or_else(|| ApiError::internal("currency checked"))?;

    let opening = match &input.opening_balance {
        None => None,
        Some(o) => {
            if o.amount.currency != currency.code.as_str() {
                return Err(field_error(
                    ErrorCode::CurrencyMismatch,
                    "openingBalance.amount",
                    "currencyMismatch",
                    "the opening balance must be in the account's currency",
                ));
            }
            let amount = parse_amount("openingBalance.amount", &o.amount.amount, currency)?;
            if !amount.is_zero() {
                // The ledger's one date rule for every entry_date written (X-6).
                validate_civil_date(&o.date).map_err(opening_error)?;
            }
            (!amount.is_zero()).then(|| Opening {
                amount,
                date: opening_date.unwrap_or_default(),
            })
        }
    };
    let row = AccountRow {
        id: existing.map_or_else(new_id, |a| a.id.clone()),
        name,
        kind: wire(input.kind),
        currency_code: currency.code.to_string(),
        scope: wire(scope),
        // A personal account keeps its owner; a new one (or one becoming personal) is mine.
        owner_member_id: (scope == AccountScope::Personal).then(|| {
            existing
                .and_then(|a| a.owner_member_id.clone())
                .unwrap_or_else(|| me.to_owned())
        }),
        visibility: wire(visibility),
        institution,
        number_mask,
        is_archived: existing.is_some_and(|a| a.is_archived),
        sort_order: existing.map_or(0, |a| a.sort_order),
    };
    if let Some(o) = &opening {
        // Expanding checks the date and the legs before anything is written.
        opening_tx(&row, o).expand().map_err(opening_error)?;
    }
    Ok((row, opening))
}

fn opening_tx(row: &AccountRow, o: &Opening) -> NewTx {
    NewTx::OpeningBalance {
        account: row.id.clone(),
        amount: o.amount,
        date: o.date.clone(),
    }
}

fn opening_error(e: DomainError) -> ApiError {
    match e {
        DomainError::InvalidDate(_) => field_error(
            ErrorCode::Validation,
            "openingBalance.date",
            "invalidDate",
            e,
        ),
        e => domain_error(e),
    }
}

/// The ledger as the account about to be written sees it (X-6: a live account only).
fn context(row: &AccountRow) -> Result<LedgerContext, ApiError> {
    let mut ctx = LedgerContext::default();
    ctx.accounts.insert(
        row.id.clone(),
        AccountInfo {
            currency: currency_code(&row.currency_code)?,
            archived: row.is_archived,
        },
    );
    Ok(ctx)
}

fn insert_opening(w: &Write<'_>, row: &AccountRow, o: &Opening) -> Result<(), ApiError> {
    let entry = opening_tx(row, o).expand().map_err(opening_error)?;
    entry.validate(&context(row)?).map_err(domain_error)?;
    let e = EntryRow {
        id: new_id(),
        entry_date: o.date.clone(),
        kind: OPENING_BALANCE.into(),
        description: String::new(),
        memo: String::new(),
        deleted: false,
    };
    let postings: Vec<PostingRow> = entry
        .postings()
        .iter()
        .map(|p| posting_row(new_id(), &e.id, p, LegStatus::Cleared))
        .collect();
    w.insert_entry(&e, &postings, row.log_visibility())?;
    Ok(())
}

/// Rewrites the stored entry in place as `o`, keeping its posting ids, and makes it live.
fn rewrite_opening(
    w: &Write<'_>,
    row: &AccountRow,
    s: StoredOpening,
    o: &Opening,
) -> Result<(), ApiError> {
    let legs = s.legs()?;
    let plan = plan_update(&legs, &opening_tx(row, o), &context(row)?).map_err(opening_error)?;
    let vis = row.log_visibility();
    w.update_entry(
        &EntryRow {
            entry_date: o.date.clone(),
            deleted: false,
            ..s.entry.clone()
        },
        vis,
    )?;
    for leg in [plan.account_leg, plan.counter_leg] {
        let status = legs
            .iter()
            .find(|l| l.id == leg.id)
            .and_then(|l| l.status)
            .unwrap_or(LegStatus::Cleared);
        w.update_posting(&posting_row(leg.id, &s.entry.id, &leg.posting, status), vis)?;
    }
    Ok(())
}

/// Soft-deletes the entry and its postings (moving their currency with the account's, so a
/// later restore stays valid).
fn remove_opening(w: &Write<'_>, row: &AccountRow, s: StoredOpening) -> Result<(), ApiError> {
    if !s.entry.deleted {
        ensure_unlocked(&s.legs()?).map_err(domain_error)?;
    }
    let vis = row.log_visibility();
    w.update_entry(
        &EntryRow {
            deleted: true,
            ..s.entry.clone()
        },
        vis,
    )?;
    for p in &s.postings {
        w.update_posting(
            &PostingRow {
                deleted: true,
                currency_code: row.currency_code.clone(),
                ..p.clone()
            },
            vis,
        )?;
    }
    Ok(())
}

fn account_dto(a: &AccountRow) -> Result<AccountDto, ApiError> {
    Ok(AccountDto {
        id: a.id.clone(),
        name: a.name.clone(),
        kind: from_wire(&a.kind)?,
        currency: a.currency_code.clone(),
        scope: from_wire(&a.scope)?,
        owner_member_id: a.owner_member_id.clone(),
        visibility: from_wire(&a.visibility)?,
        institution: a.institution.clone(),
        number_mask: a.number_mask.clone(),
        is_archived: a.is_archived,
        sort_order: sort_order(a.sort_order)?,
    })
}
