//! Journal entries and their postings: the one insert path every ledger write uses
//! (opening balances and transactions), and the reads the transaction commands page through. What to write is decided in `mrk-domain`;
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

/// Which live entries a page holds, newest first on `(entry_date DESC, id DESC)` (design Q8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntryFilter {
    /// Only entries with a live account leg on this account.
    pub account_id: Option<String>,
    /// Inclusive civil dates.
    pub from: Option<String>,
    pub to: Option<String>,
    /// Only entries with a live account leg on an account of this scope (and owner).
    pub scope: Option<ScopeFilter>,
    /// Keyset: only entries strictly after `(entry_date, id)` in page order.
    pub after: Option<(String, String)>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeFilter {
    /// `household` or `personal`.
    pub scope: String,
    pub owner_member_id: Option<String>,
}

const ENTRY_COLS: &str = "e.id, e.entry_date, e.kind, e.description, e.memo, e.deleted";

fn entry_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<EntryRow> {
    Ok(EntryRow {
        id: r.get(0)?,
        entry_date: r.get(1)?,
        kind: r.get(2)?,
        description: r.get(3)?,
        memo: r.get(4)?,
        deleted: r.get(5)?,
    })
}

const POSTING_COLS: &str = "id, entry_id, leg, account_id, category_id, amount_minor, \
                            currency_code, status, deleted";

fn posting_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PostingRow> {
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
}

impl Store {
    /// An entry by id, live or deleted.
    pub fn entry(&self, id: &str) -> Result<Option<EntryRow>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {ENTRY_COLS} FROM journal_entry e WHERE e.id = ?1"),
                [id],
                entry_row,
            )
            .optional()?)
    }

    /// One page of live entries under `f`, at most `f.limit` rows.
    pub fn entry_page(&self, f: &EntryFilter) -> Result<Vec<EntryRow>> {
        let (after_date, after_id) = f.after.clone().unzip();
        let (scope, owner) = f
            .scope
            .clone()
            .map(|s| (s.scope, s.owner_member_id))
            .unzip();
        let mut stmt = self.conn.prepare(&page_sql(f.account_id.is_some()))?;
        let rows = stmt
            .query_map(
                rusqlite::params![
                    f.from,
                    f.to,
                    after_date,
                    after_id,
                    f.account_id,
                    scope,
                    owner.flatten(),
                    f.limit
                ],
                entry_row,
            )?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// Every posting (live or deleted) of the given entries, by entry then id.
    pub fn postings_of(&self, entry_ids: &[String]) -> Result<Vec<PostingRow>> {
        if entry_ids.is_empty() {
            return Ok(Vec::new());
        }
        let marks: Vec<String> = (1..=entry_ids.len()).map(|i| format!("?{i}")).collect();
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {POSTING_COLS} FROM posting WHERE entry_id IN ({}) ORDER BY entry_id, id",
            marks.join(", ")
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(entry_ids), posting_row)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// The opening-balance entry of an account, live or deleted (one per account; the live
    /// one first if sync ever delivered two).
    pub fn opening_balance_entry(&self, account_id: &str) -> Result<Option<EntryRow>> {
        Ok(self
            .conn
            .query_row(
                &format!(
                    "SELECT {ENTRY_COLS} FROM journal_entry e \
                     WHERE kind = 'opening_balance' AND EXISTS (SELECT 1 FROM posting p \
                         WHERE p.entry_id = e.id AND p.leg = 'account' AND p.account_id = ?1) \
                     ORDER BY deleted, id LIMIT 1"
                ),
                [account_id],
                entry_row,
            )
            .optional()?)
    }

    /// Every posting of an entry, live or deleted, by id.
    pub fn entry_postings(&self, entry_id: &str) -> Result<Vec<PostingRow>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {POSTING_COLS} FROM posting WHERE entry_id = ?1 ORDER BY id"
        ))?;
        let rows = stmt
            .query_map([entry_id], posting_row)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }
}

/// The page query. With an account, the candidates come from that account's live legs
/// (`ix_posting_account`), so a rarely used account doesn't scan every entry (F6).
fn page_sql(by_account: bool) -> String {
    let account = if by_account {
        "e.id IN (SELECT p.entry_id FROM posting p \
             WHERE p.account_id = ?5 AND p.deleted = 0 AND p.leg = 'account')"
    } else {
        "?5 IS NULL"
    };
    format!(
        "SELECT {ENTRY_COLS} FROM journal_entry e \
         WHERE e.deleted = 0 AND {account} \
           AND (?1 IS NULL OR e.entry_date >= ?1) \
           AND (?2 IS NULL OR e.entry_date <= ?2) \
           AND (?3 IS NULL OR (e.entry_date, e.id) < (?3, ?4)) \
           AND (?6 IS NULL OR EXISTS (SELECT 1 FROM posting p \
                 JOIN account a ON a.id = p.account_id \
                 WHERE p.entry_id = e.id AND p.deleted = 0 AND p.leg = 'account' \
                   AND a.scope = ?6 AND a.owner_member_id IS ?7)) \
         ORDER BY e.entry_date DESC, e.id DESC LIMIT ?8"
    )
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

#[cfg(test)]
mod tests {
    use super::page_sql;
    use crate::Store;

    fn plan(sql: &str) -> String {
        let mut s = Store::open_in_memory().unwrap();
        s.migrate().unwrap();
        let mut stmt = s
            .connection()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        let rows: Vec<String> = stmt
            .query_map(
                rusqlite::params![
                    None::<String>,
                    None::<String>,
                    None::<String>,
                    None::<String>,
                    "acc",
                    None::<String>,
                    None::<String>,
                    101
                ],
                |r| r.get::<_, String>(3),
            )
            .unwrap()
            .map(Result::unwrap)
            .collect();
        rows.join("\n")
    }

    #[test]
    fn an_account_page_reads_that_accounts_legs_through_its_index_instead_of_scanning_entries() {
        let p = plan(&page_sql(true));
        // Candidates come from the account's legs; no walk over every entry by date.
        assert!(!p.contains("SCAN e"), "{p}");
        assert!(p.contains("ix_posting_account"), "{p}");
    }
}
