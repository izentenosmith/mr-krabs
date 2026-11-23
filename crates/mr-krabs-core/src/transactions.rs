//! Transactions (REQ-3–REQ-6): two-leg expense, income and transfer entries written through
//! the ledger rules in `mrk-domain` and the store's one entry path (design Q1, Q6, Q8).
//! Opening-balance entries are listed and read here but belong to their account (S4).

use std::collections::BTreeMap;

use mrk_domain::{
    AccountInfo, DomainError, EntryKind, JournalEntry, LedgerContext, Leg, LegStatus, NewTx,
    Posting, StoredLeg, TxType, classify, ensure_unlocked, plan_update, roles, validate_civil_date,
};
use mrk_money::Money;
use mrk_store::{EntryFilter, EntryRow, PostingRow, ScopeFilter, Visibility};

use crate::accounts::OPENING_BALANCE;
use crate::api::{self, NewTxInput, Page, TxDetailDto, TxKind, TxQuery, TxRowDto, View};
use crate::support::{
    Fields, check_id, command, currency_code, domain_error, field_error, new_id, parse_amount,
    posting_row, stored_leg,
};
use crate::{ApiError, App, ErrorCode, MoneyDto};

const DESCRIPTION_MAX: usize = 500;
const MEMO_MAX: usize = 2_000;
const REGULAR: &str = "regular";
const LIMIT_DEFAULT: u32 = 100;
const LIMIT_MAX: u32 = 500;
/// Separates the date and the id in a page cursor.
const CURSOR_SEP: char = '~';
/// MASTER §2: expenses lead with U+2212, never an ASCII hyphen.
const MINUS: char = '\u{2212}';

/// A checked `NewTxInput`: the transaction, its legs and its header.
struct Checked {
    tx: NewTx,
    entry: JournalEntry,
    date: String,
    description: String,
    memo: String,
}

impl App {
    pub fn tx_create(&mut self, input: NewTxInput) -> Result<TxDetailDto, ApiError> {
        command("tx_create", || {
            let c = check_input(&input)?;
            let row = EntryRow {
                id: new_id(),
                entry_date: c.date,
                kind: REGULAR.into(),
                description: c.description,
                memo: c.memo,
                deleted: false,
            };
            self.insert_tx(Some(&input), &row, &c.entry)?;
            tracing::debug!(entry = %row.id, "transaction created");
            self.detail(&row.id)
        })
    }

    pub fn tx_get(&self, id: String) -> Result<TxDetailDto, ApiError> {
        command("tx_get", || self.detail(&id))
    }

    pub fn tx_list(&self, query: TxQuery) -> Result<Page<TxRowDto>, ApiError> {
        command("tx_list", || {
            let limit = query.limit.unwrap_or(LIMIT_DEFAULT);
            let Some(filter) = self.list_filter(&query, limit)? else {
                return Ok(Page {
                    items: Vec::new(),
                    next_cursor: None,
                });
            };
            let mut entries = self.store.entry_page(&filter)?;
            let next_cursor = (entries.len() > limit as usize).then(|| {
                entries.truncate(limit as usize);
                entries
                    .last()
                    .map(|e| format!("{}{CURSOR_SEP}{}", e.entry_date, e.id))
                    .unwrap_or_default()
            });
            let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
            let mut legs: BTreeMap<String, Vec<StoredLeg>> = BTreeMap::new();
            for p in self.store.postings_of(&ids)?.iter().filter(|p| !p.deleted) {
                legs.entry(p.entry_id.clone())
                    .or_default()
                    .push(stored_leg(p)?);
            }
            let names = Names {
                accounts: self
                    .store
                    .accounts()?
                    .into_iter()
                    .map(|a| (a.id, a.name))
                    .collect(),
                categories: self
                    .store
                    .category_rows()?
                    .into_iter()
                    .map(|c| (c.id, c.name))
                    .collect(),
            };
            let items = entries
                .iter()
                .map(|e| {
                    let legs = legs.get(&e.id).map(Vec::as_slice).unwrap_or_default();
                    tx_row(e, legs, query.account_id.as_deref(), &names)
                })
                .collect::<Result<_, _>>()?;
            Ok(Page { items, next_cursor })
        })
    }

    pub fn tx_update(&mut self, id: String, input: NewTxInput) -> Result<TxDetailDto, ApiError> {
        command("tx_update", || {
            let (entry, legs) = self.editable(&id)?;
            if entry.deleted {
                return Err(not_found(&id));
            }
            // Locked before anything else: a reconciled entry answers CONFLICT whatever the
            // edit, header-only included (X-6 F1).
            ensure_unlocked(&legs).map_err(domain_error)?;
            // The header is checked even when the legs don't change (X-6 F15).
            let c = check_input(&input)?;
            let ctx = self.ledger_context(&c.entry.postings())?;
            let plan = plan_update(&legs, &c.tx, &ctx).map_err(|e| tx_error(Some(&input), e))?;
            if !plan.counter_leg.changes.is_empty() {
                self.check_category(&plan.entry)?;
            }
            let vis = self.visibility(&plan.entry.postings())?;
            self.store.write(|w| {
                w.update_entry(
                    &EntryRow {
                        entry_date: c.date,
                        description: c.description,
                        memo: c.memo,
                        ..entry.clone()
                    },
                    vis,
                )?;
                // In place, ids kept (Q6). A leg that becomes an account leg is cleared; one
                // that stops being one loses its status (X-6 F4, `posting_row`).
                for leg in [&plan.account_leg, &plan.counter_leg] {
                    let status = legs
                        .iter()
                        .find(|l| l.id == leg.id)
                        .and_then(|l| l.status)
                        .unwrap_or(LegStatus::Cleared);
                    w.update_posting(
                        &posting_row(leg.id.clone(), &entry.id, &leg.posting, status),
                        vis,
                    )?;
                }
                Ok::<_, ApiError>(())
            })?;
            tracing::debug!(entry = %id, "transaction updated");
            self.detail(&id)
        })
    }

    pub fn tx_delete(&mut self, id: String) -> Result<(), ApiError> {
        command("tx_delete", || {
            let (entry, legs) = self.editable(&id)?;
            if entry.deleted {
                return Ok(());
            }
            ensure_unlocked(&legs).map_err(domain_error)?;
            self.set_deleted(&entry, &legs, true)
        })
    }

    pub fn tx_restore(&mut self, id: String) -> Result<(), ApiError> {
        command("tx_restore", || {
            let (entry, legs) = self.editable(&id)?;
            if !entry.deleted {
                return Ok(());
            }
            ensure_unlocked(&legs).map_err(domain_error)?;
            // The accounts and category may have changed since: the entry must still be one
            // that could be written today.
            let (account, counter) = roles(&legs).map_err(internal)?;
            let journal = JournalEntry {
                tx_type: match tx_kind(&entry, &legs)? {
                    TxKind::Expense => TxType::Expense,
                    TxKind::Income => TxType::Income,
                    TxKind::Transfer => TxType::Transfer,
                    TxKind::OpeningBalance => TxType::OpeningBalance,
                },
                account_leg: account.posting.clone(),
                counter_leg: counter.posting.clone(),
            };
            journal
                .validate(&self.ledger_context(&journal.postings())?)
                .map_err(|e| tx_error(None, e))?;
            self.check_category(&journal)?;
            self.set_deleted(&entry, &legs, false)
        })
    }

    /// A regular entry the `tx_*` commands may change: opening balances belong to their
    /// account (`account_update`, Q-transactions-2).
    fn editable(&self, id: &str) -> Result<(EntryRow, Vec<StoredLeg>), ApiError> {
        let (entry, legs) = self.stored_entry(id)?;
        if entry.kind == OPENING_BALANCE {
            return Err(ApiError::new(
                ErrorCode::Validation,
                "an opening balance is edited on its account",
            ));
        }
        Ok((entry, legs))
    }

    /// Soft-deletes or restores the entry and its postings in one transaction (FR-TX-08).
    fn set_deleted(
        &self,
        entry: &EntryRow,
        legs: &[StoredLeg],
        deleted: bool,
    ) -> Result<(), ApiError> {
        let postings: Vec<Posting> = legs.iter().map(|l| l.posting.clone()).collect();
        let vis = self.visibility(&postings)?;
        let rows = self.store.entry_postings(&entry.id)?;
        self.store.write(|w| {
            w.update_entry(
                &EntryRow {
                    deleted,
                    ..entry.clone()
                },
                vis,
            )?;
            for p in &rows {
                w.update_posting(
                    &PostingRow {
                        deleted,
                        ..p.clone()
                    },
                    vis,
                )?;
            }
            Ok::<_, ApiError>(())
        })?;
        tracing::debug!(entry = %entry.id, deleted, "transaction deleted flag set");
        Ok(())
    }

    /// Checks a `tx_list` query and turns it into the store's page filter; `None` when the
    /// query can match nothing (a personal view before there is a member).
    fn list_filter(&self, query: &TxQuery, limit: u32) -> Result<Option<EntryFilter>, ApiError> {
        let mut f = Fields::default();
        if !(1..=LIMIT_MAX).contains(&limit) {
            f.add("limit", "outOfRange");
        }
        for (field, date) in [("from", &query.from), ("to", &query.to)] {
            if date
                .as_deref()
                .is_some_and(|d| validate_civil_date(d).is_err())
            {
                f.add(field, "invalidDate");
            }
        }
        let after = query.cursor.as_deref().map(decode_cursor);
        if after.as_ref().is_some_and(Option::is_none) {
            f.add("cursor", "invalid");
        }
        f.finish()?;
        if let Some(a) = &query.account_id {
            check_id(a)?;
            if self.store.account(a)?.is_none() {
                return Err(ApiError::new(
                    ErrorCode::NotFound,
                    format!("no account {a}"),
                ));
            }
        }
        let scope = match query.view {
            View::Combined => None,
            View::Household => Some(ScopeFilter {
                scope: "household".into(),
                owner_member_id: None,
            }),
            View::Personal => match self.store.local_member_id()? {
                Some(me) => Some(ScopeFilter {
                    scope: "personal".into(),
                    owner_member_id: Some(me),
                }),
                None => return Ok(None),
            },
        };
        Ok(Some(EntryFilter {
            account_id: query.account_id.clone(),
            from: query.from.clone(),
            to: query.to.clone(),
            scope,
            after: after.flatten(),
            // One more than the page: its presence says there is a next page.
            limit: limit + 1,
        }))
    }

    /// Validates `entry` against the live accounts and categories it posts to and writes it,
    /// with both postings, in one transaction. A refused entry writes nothing.
    fn insert_tx(
        &self,
        input: Option<&NewTxInput>,
        row: &EntryRow,
        entry: &JournalEntry,
    ) -> Result<(), ApiError> {
        let postings = entry.postings();
        entry
            .validate(&self.ledger_context(&postings)?)
            .map_err(|e| tx_error(input, e))?;
        self.check_category(entry)?;
        let rows: Vec<PostingRow> = postings
            .iter()
            .map(|p| posting_row(new_id(), &row.id, p, LegStatus::Cleared))
            .collect();
        let vis = self.visibility(&postings)?;
        self.store
            .write(|w| w.insert_entry(row, &rows, vis).map_err(ApiError::from))
    }

    /// The accounts the postings touch, live ones only (X-6): an unknown id stays out, so
    /// validation answers `UnknownAccount`.
    fn ledger_context(&self, postings: &[Posting]) -> Result<LedgerContext, ApiError> {
        let mut ctx = LedgerContext::default();
        for p in postings {
            if let Leg::Account(id) = &p.leg {
                check_id(id)?;
                if let Some(a) = self.store.account(id)? {
                    ctx.accounts.insert(
                        a.id.clone(),
                        AccountInfo {
                            currency: currency_code(&a.currency_code)?,
                            archived: a.is_archived,
                        },
                    );
                }
            }
        }
        Ok(ctx)
    }

    /// An expense or income posts to a live, unarchived category of its own kind.
    fn check_category(&self, entry: &JournalEntry) -> Result<(), ApiError> {
        let Leg::Category(id) = &entry.counter_leg.leg else {
            return Ok(());
        };
        check_id(id)?;
        let expected = match entry.tx_type {
            TxType::Income => "income",
            _ => "expense",
        };
        match self.store.category(id)? {
            Some(c) if !c.deleted => {
                if c.kind != expected {
                    Err(field_error(
                        ErrorCode::Validation,
                        "categoryId",
                        "wrongKind",
                        format!("the category is not an {expected} category"),
                    ))
                } else if c.is_archived {
                    Err(field_error(
                        ErrorCode::Validation,
                        "categoryId",
                        "archived",
                        "the category is archived",
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Err(field_error(
                ErrorCode::NotFound,
                "categoryId",
                "notFound",
                "no such category",
            )),
        }
    }

    /// Change-log visibility of an entry's rows (DEC-9): private when it touches a private
    /// account, otherwise household.
    fn visibility(&self, postings: &[Posting]) -> Result<Visibility, ApiError> {
        for p in postings {
            if let Leg::Account(id) = &p.leg {
                if let Some(a) = self.store.account(id)? {
                    if a.log_visibility() == Visibility::Private {
                        return Ok(Visibility::Private);
                    }
                }
            }
        }
        Ok(Visibility::Household)
    }

    /// An entry by id with the postings that share its liveness.
    fn stored_entry(&self, id: &str) -> Result<(EntryRow, Vec<StoredLeg>), ApiError> {
        check_id(id)?;
        let entry = self.store.entry(id)?.ok_or_else(|| not_found(id))?;
        let legs = self
            .store
            .entry_postings(id)?
            .iter()
            .filter(|p| p.deleted == entry.deleted)
            .map(stored_leg)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((entry, legs))
    }

    fn detail(&self, id: &str) -> Result<TxDetailDto, ApiError> {
        let (entry, legs) = self.stored_entry(id)?;
        let (account, counter) = roles(&legs).map_err(internal)?;
        let kind = tx_kind(&entry, &legs)?;
        let amount = if kind == TxKind::OpeningBalance {
            account.posting.amount
        } else {
            abs(account.posting.amount)?
        };
        Ok(TxDetailDto {
            id: entry.id,
            kind,
            date: entry.entry_date,
            description: entry.description,
            memo: entry.memo,
            account_id: target(&account.posting.leg).unwrap_or_default(),
            category_id: match &counter.posting.leg {
                Leg::Category(c) => Some(c.clone()),
                _ => None,
            },
            to_account_id: match &counter.posting.leg {
                Leg::Account(a) => Some(a.clone()),
                _ => None,
            },
            amount: MoneyDto::from_money(amount),
            status: entry_status(&legs),
            deleted: entry.deleted,
        })
    }
}

/// Display names for the rows of a page.
struct Names {
    accounts: BTreeMap<String, String>,
    categories: BTreeMap<String, String>,
}

impl Names {
    fn of(map: &BTreeMap<String, String>, id: &str) -> String {
        map.get(id).cloned().unwrap_or_default()
    }
}

/// A cursor the server issued: `<entry_date>~<id>`. Anything else is `None`.
fn decode_cursor(cursor: &str) -> Option<(String, String)> {
    let (date, id) = cursor.split_once(CURSOR_SEP)?;
    validate_civil_date(date).ok()?;
    check_id(id).ok()?;
    Some((date.to_owned(), id.to_owned()))
}

/// One row, seen from `account` when the entry has a leg on it, else from its account leg.
fn tx_row(
    entry: &EntryRow,
    legs: &[StoredLeg],
    account: Option<&str>,
    names: &Names,
) -> Result<TxRowDto, ApiError> {
    let (account_leg, counter) = roles(legs).map_err(internal)?;
    let seen_from_counter = account.is_some()
        && matches!(&counter.posting.leg, Leg::Account(a) if Some(a.as_str()) == account);
    let (seen, other) = if seen_from_counter {
        (counter, account_leg)
    } else {
        (account_leg, counter)
    };
    let kind = tx_kind(entry, legs)?;
    let account_id = target(&seen.posting.leg).unwrap_or_default();
    let (category_id, counter_account_id) = match &other.posting.leg {
        Leg::Category(c) => (Some(c.clone()), None),
        Leg::Account(a) => (None, Some(a.clone())),
        Leg::FxTrading => (None, None),
    };
    let status = seen
        .status
        .ok_or_else(|| ApiError::internal(format!("account leg {} has no status", seen.id)))?;
    Ok(TxRowDto {
        id: entry.id.clone(),
        date: entry.entry_date.clone(),
        kind,
        description: entry.description.clone(),
        account_name: Names::of(&names.accounts, &account_id),
        account_id,
        category_name: category_id
            .as_deref()
            .map(|c| Names::of(&names.categories, c)),
        category_id,
        counter_account_name: counter_account_id
            .as_deref()
            .map(|a| Names::of(&names.accounts, a)),
        counter_account_id,
        amount: row_amount(kind, seen.posting.amount)?,
        status: api_status(status),
    })
}

/// The row's money: `minor` signed as seen from its account; `display` per MASTER §2 (X-7):
/// expense `−$45.990`, income `+$1.000`, transfer unsigned, opening balance by its sign.
fn row_amount(kind: TxKind, amount: Money) -> Result<MoneyDto, ApiError> {
    let unsigned = abs(amount)?.display_es_cl();
    let sign = match kind {
        TxKind::Expense => Some(MINUS),
        TxKind::Income => Some('+'),
        TxKind::Transfer => None,
        TxKind::OpeningBalance if amount.minor < 0 => Some(MINUS),
        TxKind::OpeningBalance => Some('+'),
    };
    Ok(MoneyDto {
        display: sign.map_or_else(|| unsigned.clone(), |s| format!("{s}{unsigned}")),
        ..MoneyDto::from_money(amount)
    })
}

fn api_status(s: LegStatus) -> api::LegStatus {
    match s {
        LegStatus::Pending => api::LegStatus::Pending,
        LegStatus::Cleared => api::LegStatus::Cleared,
        LegStatus::Reconciled => api::LegStatus::Reconciled,
    }
}

/// Checks the header and amount, then expands the input into its legs.
fn check_input(input: &NewTxInput) -> Result<Checked, ApiError> {
    let (amount, date, description, memo) = match input {
        NewTxInput::Expense(c) | NewTxInput::Income(c) => {
            (&c.amount, &c.date, &c.description, &c.memo)
        }
        NewTxInput::Transfer(t) => (&t.amount, &t.date, &t.description, &t.memo),
    };
    let mut f = Fields::default();
    // The ledger's one date rule for every entry_date written (X-6 F19).
    if validate_civil_date(date).is_err() {
        f.add("date", "invalidDate");
    }
    let description = f.optional("description", description.as_deref(), DESCRIPTION_MAX);
    let memo = f.optional("memo", memo.as_deref(), MEMO_MAX);
    let currency = f.currency("amount", &amount.currency);
    f.finish()?;
    let currency = currency.ok_or_else(|| ApiError::internal("currency checked"))?;
    let amount = parse_amount("amount", &amount.amount, currency)?;
    let tx = match input {
        NewTxInput::Expense(c) => NewTx::Expense {
            account: checked_id(&c.account_id)?,
            category: checked_id(&c.category_id)?,
            amount,
        },
        NewTxInput::Income(c) => NewTx::Income {
            account: checked_id(&c.account_id)?,
            category: checked_id(&c.category_id)?,
            amount,
        },
        NewTxInput::Transfer(t) => NewTx::Transfer {
            from: checked_id(&t.from_account_id)?,
            to: checked_id(&t.to_account_id)?,
            amount,
        },
    };
    Ok(Checked {
        entry: tx.expand().map_err(|e| tx_error(Some(input), e))?,
        tx,
        date: date.clone(),
        description: description.unwrap_or_default(),
        memo: memo.unwrap_or_default(),
    })
}

fn checked_id(id: &str) -> Result<String, ApiError> {
    check_id(id)?;
    Ok(id.to_owned())
}

/// A ledger refusal as its wire code (design Q6), with the input field it concerns.
fn tx_error(input: Option<&NewTxInput>, e: DomainError) -> ApiError {
    let field = |reason: &str, account: &str| {
        let path = match input {
            Some(NewTxInput::Transfer(t)) if t.from_account_id == account => "fromAccountId",
            Some(NewTxInput::Transfer(_)) => "toAccountId",
            _ => "accountId",
        };
        (path, reason.to_owned())
    };
    let target = match &e {
        DomainError::NonPositiveAmount | DomainError::ZeroAmount => {
            Some(("amount", "invalidAmount".to_owned()))
        }
        DomainError::Money(_) => Some(("amount", "invalidAmount".to_owned())),
        DomainError::SameAccountTransfer => Some(("toAccountId", "sameAccount".to_owned())),
        DomainError::InvalidDate(_) => Some(("date", "invalidDate".to_owned())),
        DomainError::CurrencyMismatch { .. } => Some(("amount", "currencyMismatch".to_owned())),
        DomainError::ArchivedAccount(a) => Some(field("archived", a)),
        DomainError::UnknownAccount(a) => Some(field("notFound", a)),
        _ => None,
    };
    let api = domain_error(e);
    match (input, target) {
        (Some(_), Some((path, reason))) => api.with_field(path, format!("errors.field.{reason}")),
        _ => api,
    }
}

fn not_found(id: &str) -> ApiError {
    ApiError::new(ErrorCode::NotFound, format!("no transaction {id}"))
}

fn internal(e: DomainError) -> ApiError {
    ApiError::internal(e)
}

fn target(leg: &Leg) -> Option<String> {
    match leg {
        Leg::Account(a) | Leg::Category(a) => Some(a.clone()),
        Leg::FxTrading => None,
    }
}

fn abs(m: Money) -> Result<Money, ApiError> {
    m.minor
        .checked_abs()
        .map(|minor| Money::new(minor, m.currency))
        .ok_or_else(|| ApiError::internal("amount out of range"))
}

/// What an entry is: an opening balance by its kind, otherwise derived from its legs
/// (ADR-0009: `journal_entry.kind` records intent only).
fn tx_kind(entry: &EntryRow, legs: &[StoredLeg]) -> Result<TxKind, ApiError> {
    if entry.kind == OPENING_BALANCE {
        return Ok(TxKind::OpeningBalance);
    }
    let postings: Vec<Posting> = legs.iter().map(|l| l.posting.clone()).collect();
    match classify(&postings) {
        EntryKind::Expense => Ok(TxKind::Expense),
        EntryKind::Income => Ok(TxKind::Income),
        EntryKind::Transfer => Ok(TxKind::Transfer),
        EntryKind::Mixed => Err(ApiError::internal(format!(
            "entry {} mixes income and expense",
            entry.id
        ))),
    }
}

/// An entry's status from its account legs (ADR-0009, X-4 D4): pending while any leg is
/// pending, reconciled once every leg is, cleared otherwise.
fn entry_status(legs: &[StoredLeg]) -> api::LegStatus {
    let statuses: Vec<LegStatus> = legs.iter().filter_map(|l| l.status).collect();
    api_status(if statuses.contains(&LegStatus::Pending) {
        LegStatus::Pending
    } else if !statuses.is_empty() && statuses.iter().all(|s| *s == LegStatus::Reconciled) {
        LegStatus::Reconciled
    } else {
        LegStatus::Cleared
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AccountInput, AccountKind, HouseholdInput};
    use mrk_money::Currency;
    use pretty_assertions::assert_eq;

    fn count(app: &App, table: &str) -> i64 {
        app.store
            .connection()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    /// UNBALANCED_ENTRY can't be reached through `dispatch` (a two-leg input expands
    /// balanced), so it is proven here, at the write path every transaction goes through
    /// (Q-transactions-3).
    #[test]
    fn an_unbalanced_entry_is_refused_at_the_write_path_and_writes_nothing() {
        let mut app = App::open_in_memory().unwrap();
        app.household_create(HouseholdInput {
            name: "Casa".into(),
            my_name: "Yo".into(),
            base_currency: "CLP".into(),
            locale: None,
        })
        .unwrap();
        let account = app
            .account_create(AccountInput {
                name: "Cuenta".into(),
                kind: AccountKind::Checking,
                currency: "CLP".into(),
                scope: None,
                visibility: None,
                institution: None,
                number_mask: None,
                opening_balance: None,
            })
            .unwrap()
            .id;
        let clp = |minor| Money::new(minor, Currency::CLP.code);
        let unbalanced = JournalEntry {
            tx_type: TxType::Expense,
            account_leg: Posting::account(&account, clp(-10_000)),
            counter_leg: Posting::category(&crate::seed::seed_id("expense.food"), clp(9_999)),
        };
        let row = EntryRow {
            id: new_id(),
            entry_date: "2026-11-21".into(),
            kind: REGULAR.into(),
            description: String::new(),
            memo: String::new(),
            deleted: false,
        };
        let before = (
            count(&app, "journal_entry"),
            count(&app, "posting"),
            count(&app, "change_log"),
        );
        let e = app.insert_tx(None, &row, &unbalanced).unwrap_err();
        assert_eq!(e.code, ErrorCode::UnbalancedEntry, "{e}");
        assert_eq!(
            (
                count(&app, "journal_entry"),
                count(&app, "posting"),
                count(&app, "change_log")
            ),
            before
        );
    }
}
