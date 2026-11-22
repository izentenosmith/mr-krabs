//! Accounts and the account legs balances are computed from (in Rust, never in SQL:
//! ADR-0002, ADR-0003).

use rusqlite::OptionalExtension;

use crate::{Result, Store, Visibility, Write};

/// An account as M1 reads and writes it (credit-card attributes arrive with M4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub currency_code: String,
    pub scope: String,
    pub owner_member_id: Option<String>,
    pub visibility: String,
    pub institution: Option<String>,
    pub number_mask: Option<String>,
    pub is_archived: bool,
    pub sort_order: i64,
}

/// One account leg, for balances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountLegRow {
    pub account_id: String,
    pub amount_minor: i64,
    pub currency_code: String,
    pub entry_date: String,
    /// `false` when the posting or its entry is deleted.
    pub live: bool,
}

const ACCOUNT_COLS: &str = "id, name, kind, currency_code, scope, owner_member_id, visibility, \
                            institution, number_mask, is_archived, sort_order";

fn account_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AccountRow> {
    Ok(AccountRow {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: r.get(2)?,
        currency_code: r.get(3)?,
        scope: r.get(4)?,
        owner_member_id: r.get(5)?,
        visibility: r.get(6)?,
        institution: r.get(7)?,
        number_mask: r.get(8)?,
        is_archived: r.get::<_, i64>(9)? == 1,
        sort_order: r.get(10)?,
    })
}

impl AccountRow {
    /// Change-log visibility of the account and of the entries that post to it (DEC-9).
    pub fn log_visibility(&self) -> Visibility {
        if self.visibility == "private" {
            Visibility::Private
        } else {
            Visibility::Household
        }
    }
}

impl Store {
    /// A live account by id.
    pub fn account(&self, id: &str) -> Result<Option<AccountRow>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {ACCOUNT_COLS} FROM account WHERE id = ?1 AND deleted = 0"),
                [id],
                account_row,
            )
            .optional()?)
    }

    /// Every live account, ordered by sort order, name, id.
    pub fn accounts(&self) -> Result<Vec<AccountRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ACCOUNT_COLS} FROM account WHERE deleted = 0 ORDER BY sort_order, name, id"
        ))?;
        let rows = stmt
            .query_map([], account_row)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Every account leg with its entry's date and liveness.
    pub fn account_legs(&self) -> Result<Vec<AccountLegRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.account_id, p.amount_minor, p.currency_code, e.entry_date, \
                    p.deleted = 0 AND e.deleted = 0 \
             FROM posting p JOIN journal_entry e ON e.id = p.entry_id \
             WHERE p.leg = 'account'",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(AccountLegRow {
                    account_id: r.get(0)?,
                    amount_minor: r.get(1)?,
                    currency_code: r.get(2)?,
                    entry_date: r.get(3)?,
                    live: r.get(4)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Whether any posting row (live or deleted) on the account belongs to an entry other
    /// than `except`.
    pub fn account_has_postings_outside(
        &self,
        account_id: &str,
        except: Option<&str>,
    ) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM posting WHERE account_id = ?1 AND entry_id IS NOT ?2)",
            rusqlite::params![account_id, except],
            |r| r.get(0),
        )?)
    }
}

impl Write<'_> {
    pub fn insert_account(&self, a: &AccountRow) -> Result<()> {
        self.insert("account", &a.id, a.log_visibility(), &account_cols(a))
    }

    /// Writes every column of the account; only those that changed are logged.
    pub fn update_account(&self, a: &AccountRow) -> Result<()> {
        self.update("account", &a.id, a.log_visibility(), &account_cols(a)[1..])
    }
}

fn account_cols(a: &AccountRow) -> [crate::write::Col<'_>; 11] {
    [
        ("id", &a.id),
        ("name", &a.name),
        ("kind", &a.kind),
        ("currency_code", &a.currency_code),
        ("scope", &a.scope),
        ("owner_member_id", &a.owner_member_id),
        ("visibility", &a.visibility),
        ("institution", &a.institution),
        ("number_mask", &a.number_mask),
        ("is_archived", &a.is_archived),
        ("sort_order", &a.sort_order),
    ]
}
