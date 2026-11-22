//! Journal entries and their postings: the one insert path every ledger write uses
//! (opening balances here, transactions in S5). What to write is decided in `mrk-domain`;
//! this module stores it and logs every row (ADR-0010).

use rusqlite::OptionalExtension;

use crate::{Result, Store, Visibility, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryRow {
    pub id: String,
    pub entry_date: String,
    /// `entry_kind` code: `regular`, `opening_balance`, …
    pub kind: String,
    pub description: String,
    pub memo: String,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostingRow {
    pub id: String,
    pub entry_id: String,
    /// `account`, `category` or `fx` (ADR-0009).
    pub leg: String,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub amount_minor: i64,
    pub currency_code: String,
    /// Account legs only: `pending`, `cleared` or `reconciled`.
    pub status: Option<String>,
    pub deleted: bool,
}

impl Store {
    /// The opening-balance entry of an account, live or deleted (one per account; the live
    /// one first if sync ever delivered two).
    pub fn opening_balance_entry(&self, account_id: &str) -> Result<Option<EntryRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, entry_date, kind, description, memo, deleted FROM journal_entry e \
                 WHERE kind = 'opening_balance' AND EXISTS (SELECT 1 FROM posting p \
                     WHERE p.entry_id = e.id AND p.leg = 'account' AND p.account_id = ?1) \
                 ORDER BY deleted, id LIMIT 1",
                [account_id],
                |r| {
                    Ok(EntryRow {
                        id: r.get(0)?,
                        entry_date: r.get(1)?,
                        kind: r.get(2)?,
                        description: r.get(3)?,
                        memo: r.get(4)?,
                        deleted: r.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    /// Every posting of an entry, live or deleted, by id.
    pub fn entry_postings(&self, entry_id: &str) -> Result<Vec<PostingRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entry_id, leg, account_id, category_id, amount_minor, currency_code, \
                    status, deleted \
             FROM posting WHERE entry_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([entry_id], |r| {
                Ok(PostingRow {
                    id: r.get(0)?,
                    entry_id: r.get(1)?,
                    leg: r.get(2)?,
                    account_id: r.get(3)?,
                    category_id: r.get(4)?,
                    amount_minor: r.get(5)?,
                    currency_code: r.get(6)?,
                    status: r.get(7)?,
                    deleted: r.get(8)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }
}

impl Write<'_> {
    /// Inserts an entry and its postings, each logged as created.
    pub fn insert_entry(
        &self,
        entry: &EntryRow,
        postings: &[PostingRow],
        visibility: Visibility,
    ) -> Result<()> {
        self.insert("journal_entry", &entry.id, visibility, &entry_cols(entry))?;
        for p in postings {
            self.insert("posting", &p.id, visibility, &posting_cols(p))?;
        }
        Ok(())
    }

    /// Writes an existing entry's columns; only changed fields are logged.
    pub fn update_entry(&self, entry: &EntryRow, visibility: Visibility) -> Result<()> {
        self.update(
            "journal_entry",
            &entry.id,
            visibility,
            &entry_cols(entry)[1..],
        )
    }

    /// Writes an existing posting's columns in place (its id is kept); only changed fields
    /// are logged.
    pub fn update_posting(&self, p: &PostingRow, visibility: Visibility) -> Result<()> {
        self.update("posting", &p.id, visibility, &posting_cols(p)[1..])
    }
}

fn entry_cols(e: &EntryRow) -> [crate::write::Col<'_>; 6] {
    [
        ("id", &e.id),
        ("entry_date", &e.entry_date),
        ("kind", &e.kind),
        ("description", &e.description),
        ("memo", &e.memo),
        ("deleted", &e.deleted),
    ]
}

fn posting_cols(p: &PostingRow) -> [crate::write::Col<'_>; 9] {
    [
        ("id", &p.id),
        ("entry_id", &p.entry_id),
        ("leg", &p.leg),
        ("account_id", &p.account_id),
        ("category_id", &p.category_id),
        ("amount_minor", &p.amount_minor),
        ("currency_code", &p.currency_code),
        ("status", &p.status),
        ("deleted", &p.deleted),
    ]
}
