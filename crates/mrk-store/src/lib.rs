//! SQLite storage for Mr Krabs.
//!
//! SQLite is used **only** for storage and querying (ADR-0002): migrations contain tables,
//! indexes and integrity constraints — never triggers, views, procedures or UDFs.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

/// Ordered, embedded, forward-only migrations. The index + 1 is the schema version.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../migrations/0001_init.sql")),
    (
        "0002_ledger_legs_and_category_keys",
        include_str!("../migrations/0002_ledger_legs_and_category_keys.sql"),
    ),
];

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    SchemaTooNew {
        found: u32,
        supported: u32,
    },
    /// A migration left rows whose foreign keys point nowhere; it was rolled back.
    ForeignKeyViolation {
        migration: &'static str,
        table: String,
        violations: usize,
    },
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
            Self::ForeignKeyViolation {
                migration,
                table,
                violations,
            } => write!(
                f,
                "migration {migration} rolled back: {violations} foreign-key violation(s), first in {table}"
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
    ///
    /// Each migration runs with foreign keys off, which SQLite's table-rebuild procedure
    /// (create `<t>_new`, copy, drop, rename) requires, and `PRAGMA foreign_key_check` must
    /// come back empty before the commit. Foreign keys are switched back on afterwards, also
    /// when a migration fails.
    pub fn migrate(&mut self) -> Result<u32> {
        let current = self.schema_version()?;
        let latest = Self::latest_version();
        if current > latest {
            return Err(StoreError::SchemaTooNew {
                found: current,
                supported: latest,
            });
        }
        for (i, (name, sql)) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            // Must be set outside a transaction: SQLite ignores it inside one.
            self.conn.pragma_update(None, "foreign_keys", false)?;
            let applied = Self::apply(&mut self.conn, name, sql, (i + 1) as u32);
            self.conn.pragma_update(None, "foreign_keys", true)?;
            applied?;
        }
        self.schema_version()
    }

    fn apply(conn: &mut Connection, name: &'static str, sql: &str, version: u32) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        let violations: Vec<String> = {
            let mut stmt = tx.prepare("PRAGMA foreign_key_check")?;
            stmt.query_map([], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        if let Some(table) = violations.first() {
            // Dropping `tx` rolls the migration back.
            return Err(StoreError::ForeignKeyViolation {
                migration: name,
                table: table.clone(),
                violations: violations.len(),
            });
        }
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        Ok(())
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

    /// A store at an older schema version, built with the same runner as `migrate()`.
    fn at_version(version: u32) -> Store {
        let mut s = Store::open_in_memory().unwrap();
        for (i, (name, sql)) in MIGRATIONS.iter().enumerate().take(version as usize) {
            s.conn.pragma_update(None, "foreign_keys", false).unwrap();
            Store::apply(&mut s.conn, name, sql, (i + 1) as u32).unwrap();
            s.conn.pragma_update(None, "foreign_keys", true).unwrap();
        }
        s
    }

    fn count(s: &Store, sql: &str) -> i64 {
        s.connection().query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn foreign_keys_on(s: &Store) -> bool {
        count(s, "PRAGMA foreign_keys") == 1
    }

    /// Schema SQL without comments, quotes or layout, so a rebuilt-and-renamed table compares
    /// equal to the same `CREATE TABLE` run fresh.
    fn normalized_schema(conn: &Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .map(|row| {
                let (name, sql) = row.unwrap();
                let text: Vec<&str> = sql
                    .lines()
                    .map(|l| l.find("--").map_or(l, |i| &l[..i]))
                    .collect();
                let flat = text.join(" ").replace('"', "");
                (name, flat.split_whitespace().collect::<Vec<_>>().join(" "))
            })
            .collect()
    }

    #[test]
    fn migrates_from_empty_and_is_idempotent() {
        let mut s = migrated();
        assert_eq!(s.schema_version().unwrap(), 2);
        assert_eq!(s.migrate().unwrap(), 2);
    }

    #[test]
    fn core_tables_exist() {
        let tables = migrated().table_names().unwrap();
        for t in [
            "account",
            "account_kind",
            "category",
            "entry_kind",
            "journal_entry",
            "posting",
            "entry_share",
            "installment_plan",
            "recurring_rule",
            "recurring_rule_posting",
            "recurring_rule_share",
            "budget_line",
            "import_batch",
            "change_log",
            "field_clock",
        ] {
            assert!(tables.iter().any(|n| n == t), "missing table {t}");
        }
        assert!(!tables.iter().any(|n| n == "household_setting"));
    }

    #[test]
    fn schema_has_no_triggers_or_views() {
        assert_eq!(migrated().sql_logic_object_count().unwrap(), 0);
    }

    #[test]
    fn foreign_keys_stay_on_after_migrate() {
        assert!(foreign_keys_on(&migrated()));
    }

    /// The accepted design (docs/db-review/design/schema.sql, proposal rev 3) is the target:
    /// 0001 + 0002 must build exactly that schema.
    #[test]
    fn migrated_schema_matches_accepted_design() {
        let design = Connection::open_in_memory().unwrap();
        design
            .execute_batch(include_str!("../../../docs/db-review/design/schema.sql"))
            .unwrap();
        let migrated = migrated();
        let got = normalized_schema(migrated.connection());
        let want = normalized_schema(&design);
        let names = |v: &[(String, String)]| v.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
        pretty_assertions_free_eq(&names(&got), &names(&want));
        for ((name, a), (_, b)) in got.iter().zip(want.iter()) {
            assert_eq!(a, b, "{name} differs from the accepted design");
        }
    }

    fn pretty_assertions_free_eq(got: &[String], want: &[String]) {
        let missing: Vec<_> = want.iter().filter(|n| !got.contains(n)).collect();
        let extra: Vec<_> = got.iter().filter(|n| !want.contains(n)).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "missing {missing:?}, extra {extra:?}"
        );
    }

    /// Synthetic v1 rows in every table 0002 copies (category and recurring_rule stay empty:
    /// guards G1/G2 require it).
    const V1_ROWS: &str = "
        INSERT INTO currency VALUES ('CLP','Peso','$',0,'fiat',5,'a',0),('USD','Dollar','US$',2,'fiat',5,'a',0);
        INSERT INTO household_setting VALUES ('base_currency','USD',1,'a',0),('budget_mode','at_purchase',1,'a',0);
        INSERT INTO household VALUES ('h','Casa',1,'a',0);
        INSERT INTO member VALUES ('m','h','A',NULL,1,'a',0);
        INSERT INTO account (id,name,kind,currency_code,scope,owner_member_id,visibility,hlc,hlc_device)
            VALUES ('chk','Cuenta','checking','CLP','personal','m','private',1,'a');
        INSERT INTO account (id,name,kind,currency_code,scope,visibility,credit_limit_minor,statement_day,due_day,hlc,hlc_device)
            VALUES ('card','Tarjeta','credit_card','CLP','household','shared_full',100000,5,20,1,'a');
        INSERT INTO payee VALUES ('py','Super',NULL,1,'a',0);
        INSERT INTO tag VALUES ('tg','viaje',1,'a',0);
        INSERT INTO fx_rate VALUES ('fx','USD','CLP','2026-11-01','950.5','manual',1,'a',0);
        INSERT INTO installment_plan VALUES ('ip','card','2026-11-01',60000,'CLP',3,'0','2026-12','per_installment',1,'a',0);
        INSERT INTO import_profile VALUES
            ('pr1','Banco','banco_x','pdf','builtin:banco_x:card_statement',NULL,1,'a',0),
            ('pr2','CSV','x','csv','mapping','{}',1,'a',0);
        INSERT INTO import_batch (id,account_id,profile_id,file_name,file_sha256,extractor,status,created_at,hlc,hlc_device)
            VALUES ('ib','chk','pr2','a.csv','00','mapping','committed','2026-11-02',1,'a');
        INSERT INTO journal_entry (id,entry_date,kind,status,payee_id,is_shared,payer_member_id,import_batch_id,hlc,hlc_device)
            VALUES ('e1','2026-11-02','expense','reconciled','py',1,'m','ib',1,'a');
        INSERT INTO journal_entry (id,entry_date,kind,status,installment_plan_id,hlc,hlc_device)
            VALUES ('e2','2026-11-01','installment_purchase','pending','ip',1,'a');
        INSERT INTO journal_entry (id,entry_date,kind,status,hlc,hlc_device) VALUES ('e3','2026-11-03','transfer','cleared',1,'a');
        INSERT INTO posting (id,entry_id,account_id,amount_minor,currency_code,hlc,hlc_device) VALUES
            ('p1','e1','chk',-1000,'CLP',1,'a'),('p3','e2','card',-60000,'CLP',1,'a'),
            ('p5','e3','chk',-5,'CLP',1,'a'),('p6','e3','card',5,'CLP',1,'a');
        INSERT INTO installment VALUES ('in1','ip',1,'2026-12',20000,NULL,1,'a',0);
        INSERT INTO reconciliation VALUES ('r','chk','2026-11-30',0,'open',1,'a',0);
        INSERT INTO entry_share (id,entry_id,member_id,amount_minor,hlc,hlc_device) VALUES ('s1','e1','m',500,1,'a');
        INSERT INTO entry_tag VALUES ('e1','tg',1,'a',0);
        INSERT INTO payee_rule (id,match_kind,pattern,set_payee_id,hlc,hlc_device) VALUES ('pr','contains','SUPER','py',1,'a');
        INSERT INTO sync_conflict VALUES ('c1','posting','p1','edit_vs_delete','{}',0,'2026-11-03');
    ";

    fn v1_with(extra: &str) -> Store {
        let s = at_version(1);
        s.connection().execute_batch(V1_ROWS).unwrap();
        s.connection().execute_batch(extra).unwrap();
        s
    }

    fn strings(s: &Store, sql: &str) -> Vec<String> {
        let mut stmt = s.connection().prepare(sql).unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn migration_0002_carries_v1_rows() {
        let mut s = v1_with("");
        assert_eq!(s.migrate().unwrap(), 2);
        assert_eq!(
            strings(
                &s,
                "SELECT id || ':' || kind FROM journal_entry ORDER BY id"
            ),
            ["e1:regular", "e2:installment_purchase", "e3:regular"]
        );
        assert_eq!(
            strings(
                &s,
                "SELECT id || ':' || leg || ':' || status FROM posting ORDER BY id"
            ),
            [
                "p1:account:cleared",
                "p3:account:pending",
                "p5:account:cleared",
                "p6:account:cleared"
            ]
        );
        assert_eq!(
            strings(
                &s,
                "SELECT base_currency_code || ':' || budget_mode || ':' || locale FROM household"
            ),
            ["USD:at_purchase:es-CL"]
        );
        assert_eq!(
            strings(
                &s,
                "SELECT id || ':' || parser_kind || ':' || ifnull(bank,'') || ':' || ifnull(doc_type,'') \
                 FROM import_profile ORDER BY id"
            ),
            ["pr1:builtin:banco_x:card_statement", "pr2:mapping:x:"]
        );
        assert_eq!(
            strings(&s, "SELECT currency_code FROM entry_share"),
            ["CLP"]
        );
        assert_eq!(
            strings(&s, "SELECT code FROM currency ORDER BY code"),
            ["CLF", "CLP", "EUR", "USD"]
        );
        // The household's own currency row is kept, not overwritten by the seed.
        assert_eq!(count(&s, "SELECT hlc FROM currency WHERE code = 'CLP'"), 5);
        assert_eq!(count(&s, "SELECT count(*) FROM temp.sqlite_master"), 0);
        assert_eq!(
            count(&s, "SELECT count(*) FROM pragma_foreign_key_check"),
            0
        );
        assert!(foreign_keys_on(&s));
    }

    #[test]
    fn migration_0002_guards_roll_back_to_v1() {
        let cases = [
            (
                "INSERT INTO category (id,kind,name,hlc,hlc_device) VALUES ('c','expense','X',1,'a');",
                "G1:",
            ),
            (
                "INSERT INTO recurring_rule (id,name,kind,rrule,template_json,next_date,hlc,hlc_device) \
                 VALUES ('r','R','bill','R','{}','2026-12-01',1,'a');",
                "G2:",
            ),
            (
                "INSERT INTO posting (id,entry_id,account_id,amount_minor,currency_code,hlc,hlc_device) \
                 VALUES ('p9','e1','chk',3,'USD',1,'a');",
                "G3:",
            ),
            (
                "UPDATE journal_entry SET is_shared = 1 WHERE id = 'e3';",
                "G4:",
            ),
            (
                "INSERT INTO tag VALUES ('tg2','VIAJE',1,'a',0);",
                "tag.name",
            ),
            (
                "UPDATE posting SET currency_code = 'USD' WHERE entry_id = 'e3';",
                "foreign-key violation",
            ),
        ];
        for (extra, expected) in cases {
            let mut s = v1_with(extra);
            let err = s.migrate().unwrap_err().to_string();
            assert!(err.contains(expected), "{extra} → {err}");
            assert_eq!(s.schema_version().unwrap(), 1, "{extra} left a partial v2");
            assert!(
                s.table_names()
                    .unwrap()
                    .iter()
                    .any(|t| t == "household_setting")
            );
            assert!(foreign_keys_on(&s));
        }
    }

    /// One rejected write per constraint 0002 adds (design: negative-tests.sql, schema.sql part).
    #[test]
    fn schema_v2_rejects_invalid_rows() {
        let s = migrated();
        s.connection()
            .execute_batch(
                "
            INSERT INTO household (id,name,base_currency_code,hlc,hlc_device) VALUES ('h','Casa','CLP',1,'a');
            INSERT INTO member (id,household_id,display_name,hlc,hlc_device) VALUES ('m','h','A',1,'a');
            INSERT INTO account (id,name,kind,currency_code,scope,owner_member_id,visibility,hlc,hlc_device)
                VALUES ('chk','Cuenta','checking','CLP','personal','m','shared_summary',1,'a');
            INSERT INTO account (id,name,kind,currency_code,scope,visibility,hlc,hlc_device)
                VALUES ('sav','Ahorro','savings','CLP','household','shared_full',1,'a');
            INSERT INTO category (id,parent_id,kind,name,name_key,seed_key,hlc,hlc_device)
                VALUES ('food',NULL,'expense','Comida','comida','expense.food',0,'seed');
            INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device)
                VALUES ('super','food','expense','Supermercado','supermercado',0,'seed'),
                       ('deep','super','expense','Orgánico','organico',1,'a');
            INSERT INTO journal_entry (id,entry_date,hlc,hlc_device) VALUES ('e','2026-11-01',1,'a');
            INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,hlc,hlc_device)
                VALUES ('p1','e','account','chk',-100,'CLP','cleared',1,'a');
            INSERT INTO posting (id,entry_id,leg,category_id,amount_minor,currency_code,hlc,hlc_device)
                VALUES ('p2','e','category','food',100,'CLP',1,'a');
            INSERT INTO posting (id,entry_id,leg,amount_minor,currency_code,hlc,hlc_device)
                VALUES ('p3','e','fx',5,'USD',1,'a');
            INSERT INTO reconciliation (id,account_id,statement_date,statement_balance_minor,status,hlc,hlc_device)
                VALUES ('r','chk','2026-11-30',0,'open',1,'a');
            INSERT INTO recurring_rule (id,name,kind,rrule,amount_mode,expected_minor,tolerance_pct,currency_code,next_date,hlc,hlc_device)
                VALUES ('rr','Luz','bill','FREQ=MONTHLY','approximate',30000,'10','CLP','2026-12-05',1,'a');
            ",
            )
            .unwrap();
        let rejected = [
            "INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device) VALUES ('x','food','expense','SUPERMERCADO','supermercado',1,'a')",
            "INSERT INTO category (id,parent_id,kind,name,name_key,seed_key,hlc,hlc_device) VALUES ('x',NULL,'expense','X','x','expense.food',1,'a')",
            "INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device) VALUES ('x','food','income','Y','y',1,'a')",
            "INSERT INTO category (id,parent_id,kind,name,name_key,hlc,hlc_device) VALUES ('x','x','expense','Z','z',1,'a')",
            "INSERT INTO category (id,kind,name,name_key,merged_into_id,hlc,hlc_device) VALUES ('x','expense','W','w','food',1,'a')",
            "INSERT INTO category (id,kind,name,name_key,merged_into_id,deleted,hlc,hlc_device) VALUES ('x','income','V','v','food',1,1,'a')",
            "INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('x','e','account','chk',1,'USD','cleared',1,'a')",
            "INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('x','e','account','chk',1,'CLP',1,'a')",
            "INSERT INTO posting (id,entry_id,leg,category_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('x','e','category','food',1,'CLP','cleared',1,'a')",
            "INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('x','e','fx','chk',1,'CLP','cleared',1,'a')",
            "INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,hlc,hlc_device) VALUES ('x','e','account','chk',1,'CLP','reconciled',1,'a')",
            "INSERT INTO posting (id,entry_id,leg,account_id,amount_minor,currency_code,status,reconciliation_id,hlc,hlc_device) VALUES ('x','e','account','sav',1,'CLP','reconciled','r',1,'a')",
            "INSERT INTO account (id,name,kind,currency_code,scope,visibility,credit_limit_minor,hlc,hlc_device) VALUES ('x','A','checking','CLP','household','shared_full',100,1,'a')",
            "INSERT INTO account (id,name,kind,currency_code,scope,owner_member_id,visibility,hlc,hlc_device) VALUES ('x','P','prepaid','CLP','personal','m','private',1,'a')",
            "INSERT INTO account (id,name,kind,currency_code,scope,hlc,hlc_device) VALUES ('x','H','checking','CLP','household',1,'a')",
            "INSERT INTO journal_entry (id,entry_date,hlc,hlc_device) VALUES ('x','2026-13-1',1,'a')",
            "INSERT INTO journal_entry (id,entry_date,kind,hlc,hlc_device) VALUES ('x','2026-11-02','expense',1,'a')",
            "INSERT INTO payee_rule (id,match_kind,pattern,hlc,hlc_device) VALUES ('x','contains','uber',1,'a')",
            "INSERT INTO import_profile (id,name,format,parser_kind,hlc,hlc_device) VALUES ('x','B','pdf','builtin',1,'a')",
            "INSERT INTO import_profile (id,name,format,parser_kind,hlc,hlc_device) VALUES ('x','M','csv','mapping',1,'a')",
            "INSERT INTO import_profile (id,name,format,parser_kind,hlc,hlc_device) VALUES ('x','W','csv','workbook',1,'a')",
            "INSERT INTO recurring_rule (id,name,kind,rrule,amount_mode,expected_minor,currency_code,next_date,hlc,hlc_device) VALUES ('x','A','bill','R','approximate',1,'CLP','2026-12-01',1,'a')",
            "INSERT INTO recurring_rule (id,name,kind,rrule,amount_mode,expected_min_minor,expected_max_minor,currency_code,next_date,hlc,hlc_device) VALUES ('x','B','bill','R','range',9,1,'CLP','2026-12-01',1,'a')",
            "INSERT INTO recurring_rule (id,name,kind,rrule,amount_mode,expected_minor,next_date,hlc,hlc_device) VALUES ('x','C','bill','R','exact',1,'2026-12-01',1,'a')",
            "INSERT INTO recurring_rule_posting (id,rule_id,leg,account_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('x','rr','account','chk',1,'USD',1,'a')",
            "INSERT INTO recurring_rule_share (id,rule_id,amount_minor,currency_code,hlc,hlc_device) VALUES ('x','rr',1,'CLP',1,'a')",
            "INSERT INTO sync_conflict (id,entity,entity_id,kind,details_json,created_at) VALUES ('x','e','1','other','{}','2026-11-22')",
        ];
        for sql in rejected {
            assert!(s.connection().execute(sql, []).is_err(), "accepted: {sql}");
        }
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let s = migrated();
        let err = s.connection().execute(
            "INSERT INTO posting (id, entry_id, leg, account_id, amount_minor, currency_code, status, hlc, hlc_device) \
             VALUES ('p1', 'missing-entry', 'account', 'missing-account', 100, 'CLP', 'cleared', 1, 'dev-a')",
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
