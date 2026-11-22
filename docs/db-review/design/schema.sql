-- Mr Krabs · TARGET schema (design proposal, not a migration)
-- Design: docs/db-review/design/design.md  ·  Audit: docs/db-review/analysis/
-- Engine: SQLite 3.45+ (rusqlite 0.37 bundled). Date: 2026-11-21. Mode: target state, full script.
-- This is the end state after migration 0002. The migration itself (rebuild steps) is written by dbbuild
-- from the accepted proposal; 0001_init.sql stays untouched (forward-only).
-- Rules unchanged from 0001: storage + integrity only (ADR-0002), INTEGER minor units, UUID TEXT ids,
-- hlc / hlc_device / deleted on every replicated table.
-- Changes vs 0001 are tagged with the design decision that introduces them: [D-nn].

PRAGMA foreign_keys = ON;

-- ───────────────────────── Reference & settings ─────────────────────────

CREATE TABLE currency (
    code         TEXT PRIMARY KEY CHECK (length(code) BETWEEN 3 AND 8),
    name         TEXT NOT NULL,
    symbol       TEXT NOT NULL,
    minor_units  INTEGER NOT NULL CHECK (minor_units BETWEEN 0 AND 8),
    kind         TEXT NOT NULL CHECK (kind IN ('fiat','unit_of_account')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE fx_rate (
    id           TEXT PRIMARY KEY,
    base_code    TEXT NOT NULL REFERENCES currency(code),
    quote_code   TEXT NOT NULL REFERENCES currency(code),
    rate_date    TEXT NOT NULL CHECK (rate_date GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]'),
    rate         TEXT NOT NULL,
    source       TEXT NOT NULL CHECK (source IN ('manual','import','fetched')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK (base_code <> quote_code)
) STRICT;
CREATE UNIQUE INDEX ux_fx_rate ON fx_rate(base_code, quote_code, rate_date) WHERE deleted = 0;
CREATE INDEX ix_fx_rate_quote ON fx_rate(quote_code);

-- Local-only device preferences (never synced; no references to other rows).
CREATE TABLE local_setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

-- ───────────────────────── Household, members, devices ─────────────────────────

-- [D-08] household_setting (key/value) replaced by typed, FK-checked columns.
CREATE TABLE household (
    id                 TEXT PRIMARY KEY,
    name               TEXT NOT NULL,
    base_currency_code TEXT NOT NULL REFERENCES currency(code),
    budget_mode        TEXT NOT NULL DEFAULT 'per_installment' CHECK (budget_mode IN ('at_purchase','per_installment')),
    locale             TEXT NOT NULL DEFAULT 'es-CL' CHECK (locale IN ('es-CL','en')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE member (
    id           TEXT PRIMARY KEY,
    household_id TEXT NOT NULL REFERENCES household(id),
    display_name TEXT NOT NULL,
    color        TEXT,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE INDEX ix_member_household ON member(household_id);

CREATE TABLE device (
    id             TEXT PRIMARY KEY,
    member_id      TEXT NOT NULL REFERENCES member(id),
    name           TEXT NOT NULL,
    public_key     BLOB NOT NULL,
    is_local       INTEGER NOT NULL CHECK (is_local IN (0,1)),
    paired_at      TEXT,
    revoked_at     TEXT
) STRICT;                                                   -- local-only trust store
CREATE UNIQUE INDEX ux_device_local ON device(is_local) WHERE is_local = 1;
CREATE UNIQUE INDEX ux_device_public_key ON device(public_key);               -- [D-12]
CREATE INDEX ix_device_member ON device(member_id);

-- ───────────────────────── Code-owned reference lists [D-24] ─────────────────────────
-- Values each have a Rust code path (the Rust enum is the source of behaviour). Stored as rows so the list is
-- queryable and a new value is an INSERT in a migration — not a 12-step rebuild of every table that uses it.
-- Installed by migrations only; identical on every device at the same schema version, so never synced.
-- A mrk-core test asserts the Rust enum and these rows match in both directions.

CREATE TABLE account_kind (
    code        TEXT PRIMARY KEY CHECK (length(code) BETWEEN 1 AND 32),
    sort_order  INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
INSERT INTO account_kind (code, sort_order) VALUES
    ('checking',1),('savings',2),('cuenta_vista',3),('cash',4),('credit_card',5),
    ('loan',6),('mortgage',7),('investment',8),('other_asset',9),('other_liability',10);

CREATE TABLE entry_kind (
    code        TEXT PRIMARY KEY CHECK (length(code) BETWEEN 1 AND 32),
    sort_order  INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
INSERT INTO entry_kind (code, sort_order) VALUES
    ('regular',1),('opening_balance',2),('adjustment',3),('settlement',4),('installment_purchase',5),('fx_exchange',6);

-- ───────────────────────── Accounts ─────────────────────────

CREATE TABLE account (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    kind             TEXT NOT NULL REFERENCES account_kind(code),              -- [D-24]
    currency_code    TEXT NOT NULL REFERENCES currency(code),
    scope            TEXT NOT NULL CHECK (scope IN ('personal','household')),
    owner_member_id  TEXT REFERENCES member(id),
    visibility       TEXT NOT NULL                                           -- [D-12] no default: Rust sets it per scope (T-25)
                       CHECK (visibility IN ('private','shared_summary','shared_full')),
    institution      TEXT,
    number_mask      TEXT,
    credit_limit_minor INTEGER CHECK (credit_limit_minor >= 0),
    statement_day    INTEGER CHECK (statement_day BETWEEN 1 AND 31),
    due_day          INTEGER CHECK (due_day BETWEEN 1 AND 31),
    linked_account_id TEXT REFERENCES account(id),
    is_archived      INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0,1)),
    sort_order       INTEGER NOT NULL DEFAULT 0,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    UNIQUE (id, currency_code),                                                -- [D-04] target of composite FKs
    CHECK ((scope = 'personal' AND owner_member_id IS NOT NULL) OR (scope = 'household' AND owner_member_id IS NULL)),
    CHECK (visibility = 'shared_full' OR scope = 'personal'),
    -- [D-09] credit-card attributes only on credit cards; a section never links to itself
    CHECK (kind = 'credit_card' OR (credit_limit_minor IS NULL AND statement_day IS NULL AND due_day IS NULL AND linked_account_id IS NULL)),
    CHECK (linked_account_id IS NULL OR linked_account_id <> id)
) STRICT;
CREATE INDEX ix_account_owner  ON account(owner_member_id) WHERE deleted = 0;
CREATE INDEX ix_account_linked ON account(linked_account_id);
CREATE INDEX ix_account_kind   ON account(kind);

-- ───────────────────────── Categories (user-owned data, seeded) ─────────────────────────
-- FR-BU-01 (PR 14): a tree of ANY depth; any node can take postings and budgets; nothing in code names a category.
-- [D-01] Adjacency list kept. kind consistency declared: (parent_id, kind) -> category(id, kind). kind is immutable
--        (Rust), so concurrent moves can never violate it. Cycles stay a Rust check + merger rule (D-17).
-- [D-03] seed_key = stable provenance of a seeded row ('expense.food.supermarket'); NULL for user-created.
--        Seeded rows use id = UUIDv5(MRK_SEED_NAMESPACE, seed_key) so every device seeds identical rows.
-- [D-05] name_key = normalize(name) (trim, lowercase, strip accents) written by mrk-domain on every write — the same
--        function FR-IM-07 import matching uses. Controlled redundancy: no SQLite expression can strip accents.
-- [D-18] merged_into_id = where a category deleted by merge (FR-BU-07) sends late-arriving references.

CREATE TABLE category (
    id             TEXT PRIMARY KEY,
    parent_id      TEXT,
    kind           TEXT NOT NULL CHECK (kind IN ('income','expense')),
    name           TEXT NOT NULL CHECK (length(trim(name)) > 0),
    name_key       TEXT NOT NULL CHECK (length(name_key) > 0),
    icon           TEXT,
    seed_key       TEXT UNIQUE,
    merged_into_id TEXT,
    is_archived    INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0,1)),
    sort_order     INTEGER NOT NULL DEFAULT 0,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    UNIQUE (id, kind),                                                         -- composite FK target
    FOREIGN KEY (parent_id, kind)      REFERENCES category(id, kind),          -- child kind = parent kind
    FOREIGN KEY (merged_into_id, kind) REFERENCES category(id, kind),          -- merge only within a kind
    CHECK (parent_id IS NULL OR parent_id <> id),
    CHECK (merged_into_id IS NULL OR (deleted = 1 AND merged_into_id <> id))
) STRICT;
CREATE UNIQUE INDEX ux_category_sibling_name ON category(kind, ifnull(parent_id, ''), name_key) WHERE deleted = 0;
CREATE INDEX ix_category_parent ON category(parent_id, kind);
CREATE INDEX ix_category_merged ON category(merged_into_id, kind);

CREATE TABLE payee (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL CHECK (length(trim(name)) > 0),
    default_category_id TEXT REFERENCES category(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE UNIQUE INDEX ux_payee_name ON payee(name COLLATE NOCASE) WHERE deleted = 0;               -- [D-05]
CREATE INDEX ix_payee_default_category ON payee(default_category_id);

CREATE TABLE tag (
    id    TEXT PRIMARY KEY,
    name  TEXT NOT NULL CHECK (length(trim(name)) > 0),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE UNIQUE INDEX ux_tag_name ON tag(name COLLATE NOCASE) WHERE deleted = 0;                   -- [D-05]

-- ───────────────────────── Ledger (double entry) ─────────────────────────

-- Parents referenced by journal_entry are declared first (dependency order).
CREATE TABLE installment_plan (
    id                 TEXT PRIMARY KEY,
    account_id         TEXT NOT NULL,
    purchase_date      TEXT NOT NULL CHECK (purchase_date GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]'),
    total_minor        INTEGER NOT NULL CHECK (total_minor > 0),
    currency_code      TEXT NOT NULL,
    installments       INTEGER NOT NULL CHECK (installments BETWEEN 1 AND 72),
    interest_rate      TEXT NOT NULL DEFAULT '0',
    first_due_month    TEXT NOT NULL CHECK (first_due_month GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'),
    budget_mode        TEXT NOT NULL DEFAULT 'per_installment' CHECK (budget_mode IN ('at_purchase','per_installment')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    FOREIGN KEY (account_id, currency_code) REFERENCES account(id, currency_code)   -- [D-04]
) STRICT;
CREATE INDEX ix_installment_plan_account ON installment_plan(account_id, currency_code);

-- [D-23] The template is rows, not JSON: every account/category/payee/member it names is a declared FK (T-10).
-- [D-21] Amount as the user typed it: exact | approximate (± tolerance %) | range (FR-BU-06).
CREATE TABLE recurring_rule (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    kind           TEXT NOT NULL CHECK (kind IN ('bill','income','subscription','transfer')),
    rrule          TEXT NOT NULL,
    window_days    INTEGER NOT NULL DEFAULT 0 CHECK (window_days >= 0),
    amount_mode    TEXT CHECK (amount_mode IN ('exact','approximate','range')),
    expected_minor INTEGER,
    tolerance_pct  TEXT,                              -- decimal string, e.g. '10'
    expected_min_minor INTEGER,
    expected_max_minor INTEGER,
    currency_code  TEXT REFERENCES currency(code),
    payee_id       TEXT REFERENCES payee(id),
    description    TEXT NOT NULL DEFAULT '',
    payer_member_id      TEXT REFERENCES member(id),
    attributed_member_id TEXT REFERENCES member(id),
    next_date      TEXT NOT NULL CHECK (next_date GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]'),
    auto_post      INTEGER NOT NULL DEFAULT 0 CHECK (auto_post IN (0,1)),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((amount_mode IS NULL        AND expected_minor IS NULL     AND tolerance_pct IS NULL
                                       AND expected_min_minor IS NULL AND expected_max_minor IS NULL)
        OR (amount_mode = 'exact'      AND expected_minor IS NOT NULL AND tolerance_pct IS NULL
                                       AND expected_min_minor IS NULL AND expected_max_minor IS NULL)
        OR (amount_mode = 'approximate' AND expected_minor IS NOT NULL AND tolerance_pct IS NOT NULL
                                       AND expected_min_minor IS NULL AND expected_max_minor IS NULL)
        OR (amount_mode = 'range'      AND expected_minor IS NULL     AND tolerance_pct IS NULL
                                       AND expected_min_minor IS NOT NULL AND expected_max_minor IS NOT NULL
                                       AND expected_min_minor <= expected_max_minor)),
    CHECK (amount_mode IS NULL OR currency_code IS NOT NULL)
) STRICT;
CREATE INDEX ix_recurring_rule_payee      ON recurring_rule(payee_id);
CREATE INDEX ix_recurring_rule_payer      ON recurring_rule(payer_member_id);
CREATE INDEX ix_recurring_rule_attributed ON recurring_rule(attributed_member_id);

-- [D-25] parser 'builtin:<bank>:<doc>' split into typed columns.
CREATE TABLE import_profile (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    format        TEXT NOT NULL CHECK (format IN ('csv','xlsx','pdf')),
    parser_kind   TEXT NOT NULL CHECK (parser_kind IN ('mapping','workbook','builtin','ai')),
    bank          TEXT,
    doc_type      TEXT,
    mapping_json  TEXT,                            -- column-mapping options for parser_kind = 'mapping' (opaque config)
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK (parser_kind <> 'builtin' OR (bank IS NOT NULL AND doc_type IS NOT NULL)),
    CHECK (parser_kind = 'builtin' OR doc_type IS NULL),
    CHECK (parser_kind <> 'mapping' OR mapping_json IS NOT NULL),
    CHECK (parser_kind <> 'workbook' OR format = 'xlsx')
) STRICT;
CREATE UNIQUE INDEX ux_import_profile_name ON import_profile(name COLLATE NOCASE) WHERE deleted = 0;  -- [D-05]

CREATE TABLE import_batch (
    id            TEXT PRIMARY KEY,
    account_id    TEXT REFERENCES account(id),
    profile_id    TEXT REFERENCES import_profile(id),
    file_name     TEXT NOT NULL,
    file_sha256   TEXT NOT NULL,
    extractor     TEXT NOT NULL CHECK (extractor IN ('mapping','workbook','builtin','ai')),   -- [D-20] T-28
    ai_provider   TEXT,
    status        TEXT NOT NULL CHECK (status IN ('staged','committed','discarded','reverted')),
    statement_period_start TEXT,
    statement_period_end   TEXT,
    statement_closing_minor INTEGER,
    created_at    TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK (statement_closing_minor IS NULL OR account_id IS NOT NULL)          -- closing balance needs a currency
) STRICT;
CREATE INDEX ix_import_batch_sha     ON import_batch(file_sha256);
CREATE INDEX ix_import_batch_account ON import_batch(account_id);
CREATE INDEX ix_import_batch_profile ON import_batch(profile_id);

-- [D-06] kind holds intent only; expense / income / transfer / split are derived by classify().
-- [D-07] is_shared removed: "shared" ⇔ live entry_share rows exist.
-- [D-11] status moved to posting (reconciliation is per account leg).
CREATE TABLE journal_entry (
    id               TEXT PRIMARY KEY,
    entry_date       TEXT NOT NULL CHECK (entry_date GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]'),
    kind             TEXT NOT NULL DEFAULT 'regular' REFERENCES entry_kind(code),  -- [D-24]
    payee_id         TEXT REFERENCES payee(id),
    description      TEXT NOT NULL DEFAULT '',
    memo             TEXT NOT NULL DEFAULT '',
    payer_member_id  TEXT REFERENCES member(id),
    attributed_member_id TEXT REFERENCES member(id),
    installment_plan_id TEXT REFERENCES installment_plan(id),
    recurring_rule_id   TEXT REFERENCES recurring_rule(id),
    import_batch_id  TEXT REFERENCES import_batch(id),
    fingerprint      TEXT,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((kind = 'installment_purchase') = (installment_plan_id IS NOT NULL))
) STRICT;
CREATE INDEX ix_entry_date        ON journal_entry(entry_date) WHERE deleted = 0;
CREATE INDEX ix_entry_batch       ON journal_entry(import_batch_id);
CREATE INDEX ix_entry_fingerprint ON journal_entry(fingerprint) WHERE fingerprint IS NOT NULL;
CREATE INDEX ix_entry_payee       ON journal_entry(payee_id);
CREATE INDEX ix_entry_payer       ON journal_entry(payer_member_id);
CREATE INDEX ix_entry_attributed  ON journal_entry(attributed_member_id);
CREATE INDEX ix_entry_plan_fk     ON journal_entry(installment_plan_id);   -- [D-22]
CREATE UNIQUE INDEX ux_entry_plan  ON journal_entry(installment_plan_id) WHERE installment_plan_id IS NOT NULL AND deleted = 0;  -- [D-06] one purchase entry per plan (assumption A-04)
CREATE INDEX ix_entry_recurring   ON journal_entry(recurring_rule_id);
CREATE INDEX ix_entry_kind        ON journal_entry(kind);

CREATE TABLE installment (
    id             TEXT PRIMARY KEY,
    plan_id        TEXT NOT NULL REFERENCES installment_plan(id),
    number         INTEGER NOT NULL CHECK (number >= 1),
    due_month      TEXT NOT NULL CHECK (due_month GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'),
    amount_minor   INTEGER NOT NULL CHECK (amount_minor > 0),
    paid_entry_id  TEXT REFERENCES journal_entry(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE UNIQUE INDEX ux_installment ON installment(plan_id, number) WHERE deleted = 0;
CREATE INDEX ix_installment_due  ON installment(due_month) WHERE deleted = 0;
CREATE INDEX ix_installment_plan ON installment(plan_id);                      -- [D-22]
CREATE INDEX ix_installment_paid ON installment(paid_entry_id);

CREATE TABLE reconciliation (
    id                      TEXT PRIMARY KEY,
    account_id              TEXT NOT NULL REFERENCES account(id),
    statement_date          TEXT NOT NULL CHECK (statement_date GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]'),
    statement_balance_minor INTEGER NOT NULL,
    status                  TEXT NOT NULL CHECK (status IN ('open','balanced','discrepancy')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    UNIQUE (id, account_id)                                                     -- [D-11] composite FK target
) STRICT;
CREATE INDEX ix_reconciliation_account ON reconciliation(account_id);

-- [D-04] account legs: currency must equal the account's (composite FK).
-- [D-11] status + reconciliation per account leg; reconciliation must be of the same account.
-- [D-14] leg discriminator adds the FX-trading leg (Leg::FxTrading, docs/03-data-model.md).
CREATE TABLE posting (
    id                TEXT PRIMARY KEY,
    entry_id          TEXT NOT NULL REFERENCES journal_entry(id),
    leg               TEXT NOT NULL CHECK (leg IN ('account','category','fx')),
    account_id        TEXT,
    category_id       TEXT REFERENCES category(id),
    amount_minor      INTEGER NOT NULL CHECK (amount_minor <> 0),
    currency_code     TEXT NOT NULL REFERENCES currency(code),
    memo              TEXT NOT NULL DEFAULT '',
    status            TEXT CHECK (status IN ('pending','cleared','reconciled')),
    reconciliation_id TEXT,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((leg = 'account'  AND account_id IS NOT NULL AND category_id IS NULL)
        OR (leg = 'category' AND account_id IS NULL     AND category_id IS NOT NULL)
        OR (leg = 'fx'       AND account_id IS NULL     AND category_id IS NULL)),
    CHECK ((leg = 'account') = (status IS NOT NULL)),
    CHECK (leg = 'account' OR reconciliation_id IS NULL),
    CHECK ((status = 'reconciled') = (reconciliation_id IS NOT NULL)),
    FOREIGN KEY (account_id, currency_code) REFERENCES account(id, currency_code),
    FOREIGN KEY (reconciliation_id, account_id) REFERENCES reconciliation(id, account_id)
) STRICT;
CREATE INDEX ix_posting_entry          ON posting(entry_id);
CREATE INDEX ix_posting_account        ON posting(account_id, entry_id) WHERE account_id IS NOT NULL AND deleted = 0;
CREATE INDEX ix_posting_account_fk     ON posting(account_id, currency_code);
CREATE INDEX ix_posting_category       ON posting(category_id, entry_id) WHERE category_id IS NOT NULL AND deleted = 0;
CREATE INDEX ix_posting_reconciliation ON posting(reconciliation_id, account_id);
CREATE INDEX ix_posting_category_fk    ON posting(category_id);                          -- [D-22]

CREATE TABLE external_participant (
    id    TEXT PRIMARY KEY,
    name  TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE entry_share (
    id              TEXT PRIMARY KEY,
    entry_id        TEXT NOT NULL REFERENCES journal_entry(id),
    member_id       TEXT REFERENCES member(id),
    participant_id  TEXT REFERENCES external_participant(id),
    amount_minor    INTEGER NOT NULL,
    currency_code   TEXT NOT NULL REFERENCES currency(code),                    -- [D-15]
    settled_entry_id TEXT REFERENCES journal_entry(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((member_id IS NULL) <> (participant_id IS NULL))
) STRICT;
CREATE UNIQUE INDEX ux_entry_share ON entry_share(entry_id, ifnull(member_id,''), ifnull(participant_id,''), currency_code) WHERE deleted = 0;
CREATE INDEX ix_entry_share_entry       ON entry_share(entry_id);                         -- [D-22]
CREATE INDEX ix_entry_share_member      ON entry_share(member_id);
CREATE INDEX ix_entry_share_participant ON entry_share(participant_id);
CREATE INDEX ix_entry_share_settled     ON entry_share(settled_entry_id);

-- [D-23] Template legs of a recurring rule: same leg arc and currency rule as posting, no status.
CREATE TABLE recurring_rule_posting (
    id             TEXT PRIMARY KEY,
    rule_id        TEXT NOT NULL REFERENCES recurring_rule(id),
    leg            TEXT NOT NULL CHECK (leg IN ('account','category','fx')),
    account_id     TEXT,
    category_id    TEXT REFERENCES category(id),
    amount_minor   INTEGER NOT NULL CHECK (amount_minor <> 0),
    currency_code  TEXT NOT NULL REFERENCES currency(code),
    memo           TEXT NOT NULL DEFAULT '',
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((leg = 'account'  AND account_id IS NOT NULL AND category_id IS NULL)
        OR (leg = 'category' AND account_id IS NULL     AND category_id IS NOT NULL)
        OR (leg = 'fx'       AND account_id IS NULL     AND category_id IS NULL)),
    FOREIGN KEY (account_id, currency_code) REFERENCES account(id, currency_code)
) STRICT;
CREATE INDEX ix_recurring_rule_posting_rule     ON recurring_rule_posting(rule_id);
CREATE INDEX ix_recurring_rule_posting_account  ON recurring_rule_posting(account_id, currency_code);
CREATE INDEX ix_recurring_rule_posting_category ON recurring_rule_posting(category_id);

-- [D-23] Template shares (e.g. shared rent split 50/50), same arc as entry_share.
CREATE TABLE recurring_rule_share (
    id              TEXT PRIMARY KEY,
    rule_id         TEXT NOT NULL REFERENCES recurring_rule(id),
    member_id       TEXT REFERENCES member(id),
    participant_id  TEXT REFERENCES external_participant(id),
    amount_minor    INTEGER NOT NULL,
    currency_code   TEXT NOT NULL REFERENCES currency(code),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((member_id IS NULL) <> (participant_id IS NULL))
) STRICT;
CREATE UNIQUE INDEX ux_recurring_rule_share ON recurring_rule_share(rule_id, ifnull(member_id,''), ifnull(participant_id,''), currency_code) WHERE deleted = 0;
CREATE INDEX ix_recurring_rule_share_rule        ON recurring_rule_share(rule_id);
CREATE INDEX ix_recurring_rule_share_member      ON recurring_rule_share(member_id);
CREATE INDEX ix_recurring_rule_share_participant ON recurring_rule_share(participant_id);

CREATE TABLE entry_tag (
    entry_id TEXT NOT NULL REFERENCES journal_entry(id),
    tag_id   TEXT NOT NULL REFERENCES tag(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    PRIMARY KEY (entry_id, tag_id)
) STRICT;
CREATE INDEX ix_entry_tag_tag ON entry_tag(tag_id);

CREATE TABLE payee_rule (
    id           TEXT PRIMARY KEY,
    priority     INTEGER NOT NULL DEFAULT 100,
    match_kind   TEXT NOT NULL CHECK (match_kind IN ('contains','starts_with','regex','exact')),
    pattern      TEXT NOT NULL,
    account_id   TEXT REFERENCES account(id),
    set_payee_id TEXT REFERENCES payee(id),
    set_category_id TEXT REFERENCES category(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK (set_payee_id IS NOT NULL OR set_category_id IS NOT NULL)            -- [D-12]
) STRICT;
CREATE INDEX ix_payee_rule_account  ON payee_rule(account_id);
CREATE INDEX ix_payee_rule_payee    ON payee_rule(set_payee_id);
CREATE INDEX ix_payee_rule_category ON payee_rule(set_category_id);

-- ───────────────────────── Budgets & goals ─────────────────────────

CREATE TABLE budget_line (
    id             TEXT PRIMARY KEY,
    period         TEXT NOT NULL CHECK (period GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'),
    category_id    TEXT NOT NULL REFERENCES category(id),
    scope          TEXT NOT NULL CHECK (scope IN ('personal','household')),
    member_id      TEXT REFERENCES member(id),
    amount_minor   INTEGER NOT NULL CHECK (amount_minor >= 0),
    currency_code  TEXT NOT NULL REFERENCES currency(code),
    rollover       INTEGER NOT NULL DEFAULT 0 CHECK (rollover IN (0,1)),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((scope = 'personal' AND member_id IS NOT NULL) OR (scope = 'household' AND member_id IS NULL))
) STRICT;
CREATE UNIQUE INDEX ux_budget_line ON budget_line(period, category_id, scope, ifnull(member_id,'')) WHERE deleted = 0;
CREATE INDEX ix_budget_line_category ON budget_line(category_id);
CREATE INDEX ix_budget_line_member   ON budget_line(member_id);

CREATE TABLE savings_goal (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    account_id     TEXT REFERENCES account(id),
    target_minor   INTEGER NOT NULL CHECK (target_minor > 0),
    currency_code  TEXT NOT NULL REFERENCES currency(code),
    target_date    TEXT,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE INDEX ix_savings_goal_account ON savings_goal(account_id);

-- ───────────────────────── Import staging (local-only) & attachments ─────────────────────────

CREATE TABLE import_row (
    id               TEXT PRIMARY KEY,
    batch_id         TEXT NOT NULL REFERENCES import_batch(id),
    row_no           INTEGER NOT NULL,
    raw_json         TEXT NOT NULL,
    txn_date         TEXT,
    description      TEXT,
    amount_minor     INTEGER,
    currency_code    TEXT REFERENCES currency(code),
    installment_no   INTEGER,
    installment_total INTEGER,
    suggested_category_id TEXT REFERENCES category(id),
    suggested_payee_id    TEXT REFERENCES payee(id),
    confidence       TEXT,
    duplicate_of_entry_id TEXT REFERENCES journal_entry(id),
    decision         TEXT NOT NULL DEFAULT 'pending' CHECK (decision IN ('pending','accept','skip','merge')),
    issues_json      TEXT NOT NULL DEFAULT '[]'
) STRICT;
CREATE UNIQUE INDEX ux_import_row ON import_row(batch_id, row_no);
CREATE INDEX ix_import_row_category  ON import_row(suggested_category_id);
CREATE INDEX ix_import_row_payee     ON import_row(suggested_payee_id);
CREATE INDEX ix_import_row_duplicate ON import_row(duplicate_of_entry_id);

CREATE TABLE attachment (
    id          TEXT PRIMARY KEY,
    entry_id    TEXT REFERENCES journal_entry(id),
    file_name   TEXT NOT NULL,
    sha256      TEXT NOT NULL,
    mime        TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE INDEX ix_attachment_entry ON attachment(entry_id);

-- ───────────────────────── Sync bookkeeping (local-only) ─────────────────────────

CREATE TABLE change_log (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    change_id    TEXT NOT NULL UNIQUE,
    hlc          INTEGER NOT NULL,
    device_id    TEXT NOT NULL,
    entity       TEXT NOT NULL,
    entity_id    TEXT NOT NULL,
    op           TEXT NOT NULL CHECK (op IN ('upsert','delete')),
    fields_json  TEXT NOT NULL,
    visibility   TEXT NOT NULL CHECK (visibility IN ('household','private')),
    applied_at   TEXT NOT NULL
) STRICT;
CREATE INDEX ix_change_log_origin ON change_log(device_id, hlc);
CREATE INDEX ix_change_log_entity ON change_log(entity, entity_id);

CREATE TABLE field_clock (
    entity      TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    field       TEXT NOT NULL,
    hlc         INTEGER NOT NULL,
    device_id   TEXT NOT NULL,
    PRIMARY KEY (entity, entity_id, field)
) STRICT, WITHOUT ROWID;

CREATE TABLE peer_state (
    origin_device_id TEXT PRIMARY KEY,
    max_hlc          INTEGER NOT NULL
) STRICT;

CREATE TABLE sync_conflict (
    id           TEXT PRIMARY KEY,
    entity       TEXT NOT NULL,
    entity_id    TEXT NOT NULL,
    kind         TEXT NOT NULL CHECK (kind IN
                   ('edit_vs_delete','possible_duplicate','reconciled_edit','unbalanced_entry','duplicate_name')),  -- [D-12]
    details_json TEXT NOT NULL,
    resolved     INTEGER NOT NULL DEFAULT 0 CHECK (resolved IN (0,1)),
    created_at   TEXT NOT NULL
) STRICT;
