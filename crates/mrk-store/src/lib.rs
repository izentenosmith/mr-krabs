//! SQLite storage for Mr Krabs.
//!
//! SQLite is used **only** for storage and querying (ADR-0002): migrations contain tables,
//! indexes and integrity constraints — never triggers, views, procedures or UDFs.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

/// Ordered, embedded, forward-only migrations. The index + 1 is the schema version.
pub const MIGRATIONS: &[(&str, &str)] =
    &[("0001_init", include_str!("../migrations/0001_init.sql"))];

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    SchemaTooNew { found: u32, supported: u32 },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "sqlite: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::SchemaTooNew { found, supported } => write!(
                f,
                "database schema v{found} is newer than this app supports (v{supported})"
            ),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct Store {
    conn: Connection,
    path: Option<PathBuf>,
}

impl Store {
    /// Opens (creating if needed) a database file in WAL mode with foreign keys on.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        debug_assert!(mode.eq_ignore_ascii_case("wal"));
        Self::configure(conn, Some(path.to_path_buf()))
    }

    /// In-memory database (tests).
    pub fn open_in_memory() -> Result<Self> {
        Self::configure(Connection::open_in_memory()?, None)
    }

    fn configure(conn: Connection, path: Option<PathBuf>) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self { conn, path })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Latest schema version known to this build.
    pub fn latest_version() -> u32 {
        MIGRATIONS.len() as u32
    }

    pub fn schema_version(&self) -> Result<u32> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// Applies pending migrations, each in its own transaction. Returns the new version.
    pub fn migrate(&mut self) -> Result<u32> {
        let current = self.schema_version()?;
        let latest = Self::latest_version();
        if current > latest {
            return Err(StoreError::SchemaTooNew {
                found: current,
                supported: latest,
            });
        }
        for (i, (_name, sql)) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as u32)?;
            tx.commit()?;
        }
        self.schema_version()
    }

    /// Names of user tables, sorted.
    pub fn table_names(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let names = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(names)
    }

    /// Number of triggers and views in the database — must always be zero (ADR-0002).
    pub fn sql_logic_object_count(&self) -> Result<u32> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type IN ('trigger', 'view')",
            [],
            |r| r.get(0),
        )?)
    }

    /// Raw connection for repositories inside this crate's future modules.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrated() -> Store {
        let mut s = Store::open_in_memory().unwrap();
        assert_eq!(s.migrate().unwrap(), Store::latest_version());
        s
    }

    #[test]
    fn migrates_from_empty_and_is_idempotent() {
        let mut s = migrated();
        assert_eq!(s.schema_version().unwrap(), 1);
        assert_eq!(s.migrate().unwrap(), 1);
    }

    #[test]
    fn core_tables_exist() {
        let tables = migrated().table_names().unwrap();
        for t in [
            "account",
            "category",
            "journal_entry",
            "posting",
            "entry_share",
            "installment_plan",
            "budget_line",
            "import_batch",
            "change_log",
            "field_clock",
        ] {
            assert!(tables.iter().any(|n| n == t), "missing table {t}");
        }
    }

    #[test]
    fn schema_has_no_triggers_or_views() {
        assert_eq!(migrated().sql_logic_object_count().unwrap(), 0);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let s = migrated();
        s.connection()
            .execute(
                "INSERT INTO currency (code, name, symbol, minor_units, kind, hlc, hlc_device) \
                 VALUES ('CLP', 'Peso chileno', '$', 0, 'fiat', 1, 'dev-a')",
                [],
            )
            .unwrap();
        let err = s.connection().execute(
            "INSERT INTO posting (id, entry_id, account_id, amount_minor, currency_code, hlc, hlc_device) \
             VALUES ('p1', 'missing-entry', 'missing-account', 100, 'CLP', 1, 'dev-a')",
            [],
        );
        assert!(err.is_err(), "posting with unknown entry must be rejected");
    }

    #[test]
    fn rejects_newer_schema() {
        let mut s = Store::open_in_memory().unwrap();
        s.connection()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        assert!(matches!(
            s.migrate(),
            Err(StoreError::SchemaTooNew { found: 99, .. })
        ));
    }

    #[test]
    fn opens_file_in_wal_mode() {
        let dir = std::env::temp_dir().join(format!("mrk-store-test-{}", std::process::id()));
        let path = dir.join("nested").join("mrkrabs.sqlite");
        let mut s = Store::open(&path).unwrap();
        s.migrate().unwrap();
        let mode: String = s
            .connection()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        assert_eq!(s.path(), Some(path.as_path()));
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }
}
