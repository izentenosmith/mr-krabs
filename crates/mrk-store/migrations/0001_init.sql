-- Mr Krabs · schema v1
-- Rules: storage + integrity only. NO triggers, NO views, NO procedures, NO logic.
-- Money: INTEGER minor units + currency code. Rates: TEXT decimal (parsed by rust_decimal).
-- IDs: UUIDv7 as TEXT. Sync columns on every replicated table:
--   hlc        INTEGER  -- last-writer HLC (48-bit ms << 16 | 16-bit counter)
--   hlc_device TEXT     -- device that produced `hlc` (deterministic tiebreak)
--   deleted    INTEGER  -- tombstone flag (0/1); rows are never hard-deleted while syncing

PRAGMA foreign_keys = ON;

-- ───────────────────────── Reference & local settings ─────────────────────────

CREATE TABLE currency (
    code         TEXT PRIMARY KEY CHECK (length(code) BETWEEN 3 AND 8),  -- 'CLP','USD','CLF'(UF)
    name         TEXT NOT NULL,
    symbol       TEXT NOT NULL,
    minor_units  INTEGER NOT NULL CHECK (minor_units BETWEEN 0 AND 8),    -- CLP=0, USD=2, CLF=4
    kind         TEXT NOT NULL CHECK (kind IN ('fiat','unit_of_account')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE fx_rate (
    id           TEXT PRIMARY KEY,
    base_code    TEXT NOT NULL REFERENCES currency(code),
    quote_code   TEXT NOT NULL REFERENCES currency(code),
    rate_date    TEXT NOT NULL,                 -- ISO date YYYY-MM-DD
    rate         TEXT NOT NULL,                 -- decimal string: 1 base = rate quote
    source       TEXT NOT NULL CHECK (source IN ('manual','import','fetched')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK (base_code <> quote_code)
) STRICT;
CREATE UNIQUE INDEX ux_fx_rate ON fx_rate(base_code, quote_code, rate_date) WHERE deleted = 0;

-- Local-only key/value (never synced): ui prefs, base currency per device view, AI provider config (no secrets).
CREATE TABLE local_setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

-- Household-wide settings (synced): base_currency, budget_mode, locale defaults.
CREATE TABLE household_setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

-- ───────────────────────── Household, members, devices ─────────────────────────

CREATE TABLE household (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE member (
    id           TEXT PRIMARY KEY,
    household_id TEXT NOT NULL REFERENCES household(id),
    display_name TEXT NOT NULL,
    color        TEXT,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE device (
    id             TEXT PRIMARY KEY,
    member_id      TEXT NOT NULL REFERENCES member(id),
    name           TEXT NOT NULL,
    public_key     BLOB NOT NULL,                 -- X25519 static key (Noise)
    is_local       INTEGER NOT NULL CHECK (is_local IN (0,1)),
    paired_at      TEXT,
    revoked_at     TEXT
) STRICT;                                         -- local-only (each device keeps its own trust store)
CREATE UNIQUE INDEX ux_device_local ON device(is_local) WHERE is_local = 1;

-- ───────────────────────── Accounts & categories ─────────────────────────

CREATE TABLE account (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    kind             TEXT NOT NULL CHECK (kind IN
                       ('checking','savings','cuenta_vista','cash','credit_card','loan','mortgage','investment','other_asset','other_liability')),
    currency_code    TEXT NOT NULL REFERENCES currency(code),
    scope            TEXT NOT NULL CHECK (scope IN ('personal','household')),
    owner_member_id  TEXT REFERENCES member(id),
    visibility       TEXT NOT NULL DEFAULT 'shared_summary'
                       CHECK (visibility IN ('private','shared_summary','shared_full')),
    institution      TEXT,
    number_mask      TEXT,                         -- e.g. '•••• 4417' (never full numbers)
    credit_limit_minor INTEGER,
    statement_day    INTEGER CHECK (statement_day BETWEEN 1 AND 31),
    due_day          INTEGER CHECK (due_day BETWEEN 1 AND 31),
    linked_account_id TEXT REFERENCES account(id), -- e.g. USD section of a CLP credit card
    is_archived      INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0,1)),
    sort_order       INTEGER NOT NULL DEFAULT 0,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((scope = 'personal' AND owner_member_id IS NOT NULL) OR (scope = 'household' AND owner_member_id IS NULL)),
    CHECK (visibility = 'shared_full' OR scope = 'personal')
) STRICT;
CREATE INDEX ix_account_scope ON account(scope, owner_member_id) WHERE deleted = 0;

CREATE TABLE category (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT REFERENCES category(id),     -- NULL = group
    kind        TEXT NOT NULL CHECK (kind IN ('income','expense')),
    name        TEXT NOT NULL,
    icon        TEXT,
    is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0,1)),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE INDEX ix_category_parent ON category(parent_id);

CREATE TABLE payee (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    default_category_id TEXT REFERENCES category(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE tag (
    id    TEXT PRIMARY KEY,
    name  TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

-- ───────────────────────── Ledger (double entry) ─────────────────────────
-- journal_entry = the user-facing "transaction". postings = its legs.
-- Invariant (enforced in Rust, not SQL): for each currency, SUM(postings.amount_minor) = 0,
-- where category legs act as nominal (income/expense) accounts.

CREATE TABLE journal_entry (
    id               TEXT PRIMARY KEY,
    entry_date       TEXT NOT NULL,                -- civil date YYYY-MM-DD
    kind             TEXT NOT NULL CHECK (kind IN
                       ('expense','income','transfer','split','opening_balance','adjustment','settlement','installment_purchase','fx_exchange')),
    status           TEXT NOT NULL DEFAULT 'cleared' CHECK (status IN ('pending','cleared','reconciled')),
    payee_id         TEXT REFERENCES payee(id),
    description      TEXT NOT NULL DEFAULT '',     -- original bank description
    memo             TEXT NOT NULL DEFAULT '',
    is_shared        INTEGER NOT NULL DEFAULT 0 CHECK (is_shared IN (0,1)),
    payer_member_id  TEXT REFERENCES member(id),
    attributed_member_id TEXT REFERENCES member(id), -- "whose expense" (e.g. car maintenance of member A)
    installment_plan_id TEXT REFERENCES installment_plan(id),
    recurring_rule_id   TEXT REFERENCES recurring_rule(id),
    import_batch_id  TEXT REFERENCES import_batch(id),
    fingerprint      TEXT,                         -- dedup hash computed in Rust
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE INDEX ix_entry_date       ON journal_entry(entry_date) WHERE deleted = 0;
CREATE INDEX ix_entry_batch      ON journal_entry(import_batch_id);
CREATE INDEX ix_entry_fingerprint ON journal_entry(fingerprint) WHERE fingerprint IS NOT NULL;

CREATE TABLE posting (
    id             TEXT PRIMARY KEY,
    entry_id       TEXT NOT NULL REFERENCES journal_entry(id),
    account_id     TEXT REFERENCES account(id),
    category_id    TEXT REFERENCES category(id),
    amount_minor   INTEGER NOT NULL CHECK (amount_minor <> 0),
    currency_code  TEXT NOT NULL REFERENCES currency(code),
    memo           TEXT NOT NULL DEFAULT '',
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((account_id IS NULL) <> (category_id IS NULL))   -- exactly one leg target
) STRICT;
CREATE INDEX ix_posting_entry    ON posting(entry_id);
CREATE INDEX ix_posting_account  ON posting(account_id, entry_id) WHERE account_id IS NOT NULL AND deleted = 0;
CREATE INDEX ix_posting_category ON posting(category_id, entry_id) WHERE category_id IS NOT NULL AND deleted = 0;

-- People outside the household who share some costs (e.g. a subscription split 3 ways).
CREATE TABLE external_participant (
    id    TEXT PRIMARY KEY,
    name  TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

-- Who bears the cost of a shared entry (sums to the expense amount; checked in Rust).
CREATE TABLE entry_share (
    id              TEXT PRIMARY KEY,
    entry_id        TEXT NOT NULL REFERENCES journal_entry(id),
    member_id       TEXT REFERENCES member(id),
    participant_id  TEXT REFERENCES external_participant(id),
    amount_minor    INTEGER NOT NULL,
    settled_entry_id TEXT REFERENCES journal_entry(id),  -- reimbursement / settlement that closed it
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((member_id IS NULL) <> (participant_id IS NULL))
) STRICT;
CREATE UNIQUE INDEX ux_entry_share ON entry_share(entry_id, ifnull(member_id,''), ifnull(participant_id,'')) WHERE deleted = 0;

CREATE TABLE entry_tag (
    entry_id TEXT NOT NULL REFERENCES journal_entry(id),
    tag_id   TEXT NOT NULL REFERENCES tag(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    PRIMARY KEY (entry_id, tag_id)
) STRICT;

-- Compras en cuotas. Schedule rows are generated by Rust and stored for querying.
CREATE TABLE installment_plan (
    id                 TEXT PRIMARY KEY,
    account_id         TEXT NOT NULL REFERENCES account(id),
    purchase_date      TEXT NOT NULL,
    total_minor        INTEGER NOT NULL CHECK (total_minor > 0),
    currency_code      TEXT NOT NULL REFERENCES currency(code),
    installments       INTEGER NOT NULL CHECK (installments BETWEEN 1 AND 72),
    interest_rate      TEXT NOT NULL DEFAULT '0',   -- monthly rate, decimal string
    first_due_month    TEXT NOT NULL,               -- YYYY-MM
    budget_mode        TEXT NOT NULL DEFAULT 'per_installment' CHECK (budget_mode IN ('at_purchase','per_installment')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE installment (
    id             TEXT PRIMARY KEY,
    plan_id        TEXT NOT NULL REFERENCES installment_plan(id),
    number         INTEGER NOT NULL CHECK (number >= 1),
    due_month      TEXT NOT NULL,                  -- YYYY-MM
    amount_minor   INTEGER NOT NULL CHECK (amount_minor > 0),
    paid_entry_id  TEXT REFERENCES journal_entry(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE UNIQUE INDEX ux_installment ON installment(plan_id, number) WHERE deleted = 0;
CREATE INDEX ix_installment_due ON installment(due_month) WHERE deleted = 0;

CREATE TABLE recurring_rule (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    kind           TEXT NOT NULL CHECK (kind IN ('bill','income','subscription','transfer')),
    rrule          TEXT NOT NULL,                  -- RFC 5545 subset, interpreted in Rust
    window_days    INTEGER NOT NULL DEFAULT 0 CHECK (window_days >= 0), -- e.g. salary "20th–25th" = day 20 + 5
    expected_min_minor INTEGER,                    -- expected amount / range (FR-BU-06)
    expected_max_minor INTEGER,
    currency_code  TEXT REFERENCES currency(code),
    template_json  TEXT NOT NULL,                  -- serialized NewEntry template
    next_date      TEXT NOT NULL,
    auto_post      INTEGER NOT NULL DEFAULT 0 CHECK (auto_post IN (0,1)),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE payee_rule (
    id           TEXT PRIMARY KEY,
    priority     INTEGER NOT NULL DEFAULT 100,
    match_kind   TEXT NOT NULL CHECK (match_kind IN ('contains','starts_with','regex','exact')),
    pattern      TEXT NOT NULL,
    account_id   TEXT REFERENCES account(id),      -- NULL = any account
    set_payee_id TEXT REFERENCES payee(id),
    set_category_id TEXT REFERENCES category(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

-- ───────────────────────── Budgets & goals ─────────────────────────

CREATE TABLE budget_line (
    id             TEXT PRIMARY KEY,
    period         TEXT NOT NULL,                  -- YYYY-MM
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

CREATE TABLE savings_goal (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    account_id     TEXT REFERENCES account(id),
    target_minor   INTEGER NOT NULL CHECK (target_minor > 0),
    currency_code  TEXT NOT NULL REFERENCES currency(code),
    target_date    TEXT,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE reconciliation (
    id                      TEXT PRIMARY KEY,
    account_id              TEXT NOT NULL REFERENCES account(id),
    statement_date          TEXT NOT NULL,
    statement_balance_minor INTEGER NOT NULL,
    status                  TEXT NOT NULL CHECK (status IN ('open','balanced','discrepancy')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

-- ───────────────────────── Import & staging ─────────────────────────

CREATE TABLE import_profile (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    bank          TEXT,
    format        TEXT NOT NULL CHECK (format IN ('csv','xlsx','pdf')),
    parser        TEXT NOT NULL,                   -- 'mapping' | 'builtin:<bank>:<doc>' | 'ai'
    mapping_json  TEXT,                            -- column mapping / options
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

CREATE TABLE import_batch (
    id            TEXT PRIMARY KEY,
    account_id    TEXT REFERENCES account(id),
    profile_id    TEXT REFERENCES import_profile(id),
    file_name     TEXT NOT NULL,
    file_sha256   TEXT NOT NULL,
    extractor     TEXT NOT NULL CHECK (extractor IN ('mapping','builtin','ai')),
    ai_provider   TEXT,                            -- e.g. 'ollama:qwen2.5', 'anthropic:claude-haiku-5-5'
    status        TEXT NOT NULL CHECK (status IN ('staged','committed','discarded','reverted')),
    statement_period_start TEXT,
    statement_period_end   TEXT,
    statement_closing_minor INTEGER,
    created_at    TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE INDEX ix_import_batch_sha ON import_batch(file_sha256);

-- Staging rows are local-only until committed (not synced).
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
    confidence       TEXT,                          -- decimal 0..1 (AI/heuristic)
    duplicate_of_entry_id TEXT REFERENCES journal_entry(id),
    decision         TEXT NOT NULL DEFAULT 'pending' CHECK (decision IN ('pending','accept','skip','merge')),
    issues_json      TEXT NOT NULL DEFAULT '[]'
) STRICT;
CREATE UNIQUE INDEX ux_import_row ON import_row(batch_id, row_no);

CREATE TABLE attachment (
    id          TEXT PRIMARY KEY,
    entry_id    TEXT REFERENCES journal_entry(id),
    file_name   TEXT NOT NULL,
    sha256      TEXT NOT NULL,
    mime        TEXT NOT NULL,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

-- ───────────────────────── Sync bookkeeping (local-only) ─────────────────────────

-- Append-only operation log. Every local mutation appends here in the same SQL transaction.
CREATE TABLE change_log (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,  -- local order
    change_id    TEXT NOT NULL UNIQUE,               -- UUIDv7, globally unique
    hlc          INTEGER NOT NULL,
    device_id    TEXT NOT NULL,                      -- origin device
    entity       TEXT NOT NULL,                      -- table name
    entity_id    TEXT NOT NULL,
    op           TEXT NOT NULL CHECK (op IN ('upsert','delete')),
    fields_json  TEXT NOT NULL,                      -- {"field": value, ...} changed fields
    visibility   TEXT NOT NULL CHECK (visibility IN ('household','private')),
    applied_at   TEXT NOT NULL
) STRICT;
CREATE INDEX ix_change_log_origin ON change_log(device_id, hlc);
CREATE INDEX ix_change_log_entity ON change_log(entity, entity_id);

-- Per-field LWW clock: decides whether an incoming field change wins.
CREATE TABLE field_clock (
    entity      TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    field       TEXT NOT NULL,
    hlc         INTEGER NOT NULL,
    device_id   TEXT NOT NULL,
    PRIMARY KEY (entity, entity_id, field)
) STRICT, WITHOUT ROWID;

-- What we have seen from each origin device (version vector).
CREATE TABLE peer_state (
    origin_device_id TEXT PRIMARY KEY,
    max_hlc          INTEGER NOT NULL
) STRICT;

-- Conflicts that need a human (semantic, not mechanical).
CREATE TABLE sync_conflict (
    id           TEXT PRIMARY KEY,
    entity       TEXT NOT NULL,
    entity_id    TEXT NOT NULL,
    kind         TEXT NOT NULL,                     -- 'edit_vs_delete','possible_duplicate','reconciled_edit'
    details_json TEXT NOT NULL,
    resolved     INTEGER NOT NULL DEFAULT 0 CHECK (resolved IN (0,1)),
    created_at   TEXT NOT NULL
) STRICT;
