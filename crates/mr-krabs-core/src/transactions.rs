//! Transactions (REQ-3–REQ-6): two-leg expense, income and transfer entries written through
//! the ledger rules in `mrk-domain` and the store's one entry path (design Q1, Q6, Q8).
//! Opening-balance entries are listed and read here but belong to their account (S4).

use mrk_domain::{
    AccountInfo, DomainError, EntryKind, JournalEntry, LedgerContext, Leg, LegStatus, NewTx,
    Posting, StoredLeg, TxType, classify, roles, validate_civil_date,
};
use mrk_money::Money;
use mrk_store::{EntryRow, PostingRow, Visibility};

use crate::api::{self, NewTxInput, TxDetailDto, TxKind};
use crate::support::{
    Fields, check_id, command, domain_error, field_error, new_id, parse_amount, posting_row,
    stored_leg,
};
use crate::{ApiError, App, ErrorCode, MoneyDto};

const DESCRIPTION_MAX: usize = 500;
const MEMO_MAX: usize = 2_000;
const REGULAR: &str = "regular";
const OPENING_BALANCE: &str = "opening_balance";

/// A checked `NewTxInput`: the transaction, its legs and its header.
struct Checked {
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
                            currency: crate::support::currency_code(&a.currency_code)?,
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
        let entry = self
            .store
            .entry(id)?
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, format!("no transaction {id}")))?;
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
    if statuses.contains(&LegStatus::Pending) {
        api::LegStatus::Pending
    } else if !statuses.is_empty() && statuses.iter().all(|s| *s == LegStatus::Reconciled) {
        api::LegStatus::Reconciled
    } else {
        api::LegStatus::Cleared
    }
}
