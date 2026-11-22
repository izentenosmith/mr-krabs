-- Mr Krabs · PLANNED schema delta for M4 / M5 / sync (design proposal, not a migration)
-- Runs on top of schema.sql (the 0002 target). Each block becomes its own forward migration in the milestone
-- that builds the feature (docs/07-roadmap.md); the DDL here fixes the shape so 0002 doesn't paint it into a corner.
-- Engine: SQLite 3.45+. Date: 2026-11-22. Requirements: PR 14 (b660aa1).

PRAGMA foreign_keys = ON;

-- ───────────── [D-10] Valuations: manual asset values (FR-AC-07, M4) + partner summaries (docs/05, sync) ─────────────
-- One shape for "an account's balance asserted at a date rather than derived from postings".
CREATE TABLE account_valuation (
    account_id     TEXT NOT NULL,
    as_of_date     TEXT NOT NULL CHECK (as_of_date GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]'),
    value_minor    INTEGER NOT NULL,
    currency_code  TEXT NOT NULL,
    source         TEXT NOT NULL CHECK (source IN ('manual','partner_summary')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    PRIMARY KEY (account_id, as_of_date),
    FOREIGN KEY (account_id, currency_code) REFERENCES account(id, currency_code)
) STRICT, WITHOUT ROWID;

-- ───────────── [D-19] Rules engine (FR-TX-10, M5) — replaces payee_rule ─────────────
-- Typed columns instead of JSON so every referenced row is a declared FK.
CREATE TABLE rule (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    stage       TEXT NOT NULL DEFAULT 'default' CHECK (stage IN ('pre','default','post')),
    priority    INTEGER NOT NULL DEFAULT 100,
    is_enabled  INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0,1)),
    origin      TEXT NOT NULL DEFAULT 'manual' CHECK (origin IN ('manual','learned')),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;

-- All conditions of a rule must hold (AND). "one of" = several rows with the same (rule_id, field, op='one_of').
CREATE TABLE rule_condition (
    id          TEXT PRIMARY KEY,
    rule_id     TEXT NOT NULL REFERENCES rule(id),
    field       TEXT NOT NULL CHECK (field IN ('description','payee','amount','account','date')),
    op          TEXT NOT NULL CHECK (op IN ('is','contains','matches','one_of','between')),
    text_value  TEXT,                                   -- description / regex
    payee_id    TEXT REFERENCES payee(id),
    account_id  TEXT REFERENCES account(id),
    amount_min_minor INTEGER,
    amount_max_minor INTEGER,
    currency_code TEXT REFERENCES currency(code),
    date_from   TEXT,
    date_to     TEXT,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((field = 'description' AND text_value IS NOT NULL AND op IN ('is','contains','matches','one_of'))
        OR (field = 'payee'       AND payee_id   IS NOT NULL AND op IN ('is','one_of'))
        OR (field = 'account'     AND account_id IS NOT NULL AND op IN ('is','one_of'))
        OR (field = 'amount'      AND currency_code IS NOT NULL AND op IN ('is','between')
                                  AND amount_min_minor IS NOT NULL
                                  AND (op = 'is' OR (amount_max_minor IS NOT NULL AND amount_min_minor <= amount_max_minor)))
        OR (field = 'date'        AND date_from IS NOT NULL AND op IN ('is','between')
                                  AND (op = 'is' OR (date_to IS NOT NULL AND date_from <= date_to))))
) STRICT;
CREATE INDEX ix_rule_condition_rule    ON rule_condition(rule_id);
CREATE INDEX ix_rule_condition_payee   ON rule_condition(payee_id);
CREATE INDEX ix_rule_condition_account ON rule_condition(account_id);

CREATE TABLE rule_action (
    id          TEXT PRIMARY KEY,
    rule_id     TEXT NOT NULL REFERENCES rule(id),
    action      TEXT NOT NULL CHECK (action IN ('set_category','set_payee','set_memo','add_tag','set_status')),
    category_id TEXT REFERENCES category(id),
    payee_id    TEXT REFERENCES payee(id),
    tag_id      TEXT REFERENCES tag(id),
    text_value  TEXT,
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1)),
    CHECK ((action = 'set_category' AND category_id IS NOT NULL)
        OR (action = 'set_payee'    AND payee_id    IS NOT NULL)
        OR (action = 'add_tag'      AND tag_id      IS NOT NULL)
        OR (action = 'set_memo'     AND text_value  IS NOT NULL)
        OR (action = 'set_status'   AND text_value IN ('pending','cleared')))
) STRICT;
CREATE INDEX ix_rule_action_rule     ON rule_action(rule_id);
CREATE INDEX ix_rule_action_category ON rule_action(category_id);
CREATE INDEX ix_rule_action_payee    ON rule_action(payee_id);
CREATE INDEX ix_rule_action_tag      ON rule_action(tag_id);
-- The M5 migration copies each payee_rule into rule + one condition + 1–2 actions, then drops payee_rule.

-- ───────────── [D-20] Import staging (FR-TX-07, FR-IM-06, FR-IM-07, M5) ─────────────
-- import_row: what the file said, where the suggestion came from, and the transfer target for savings rows.
ALTER TABLE import_row ADD COLUMN source_category TEXT;                      -- category text read from the file
ALTER TABLE import_row ADD COLUMN suggestion_source TEXT
    CHECK (suggestion_source IN ('file','rule','categorizer','ai'));
ALTER TABLE import_row ADD COLUMN transfer_account_id TEXT REFERENCES account(id);
CREATE INDEX ix_import_row_transfer ON import_row(transfer_account_id);

-- Duplicate candidates with a match score per existing entry (FR-TX-07). Local-only, like import_row.
CREATE TABLE import_row_candidate (
    row_id     TEXT NOT NULL REFERENCES import_row(id),
    entry_id   TEXT NOT NULL REFERENCES journal_entry(id),
    score      TEXT NOT NULL,                       -- decimal 0..1, computed in Rust
    PRIMARY KEY (row_id, entry_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX ix_import_row_candidate_entry ON import_row_candidate(entry_id);

-- Planned values from the workbook's Resumen become staged budget lines (FR-IM-06). Local-only.
CREATE TABLE import_budget_row (
    id              TEXT PRIMARY KEY,
    batch_id        TEXT NOT NULL REFERENCES import_batch(id),
    row_no          INTEGER NOT NULL,
    period          TEXT NOT NULL CHECK (period GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'),
    source_category TEXT NOT NULL,
    category_id     TEXT REFERENCES category(id),
    amount_minor    INTEGER NOT NULL CHECK (amount_minor >= 0),
    currency_code   TEXT NOT NULL REFERENCES currency(code),
    decision        TEXT NOT NULL DEFAULT 'pending' CHECK (decision IN ('pending','accept','skip'))
) STRICT;
CREATE UNIQUE INDEX ux_import_budget_row ON import_budget_row(batch_id, row_no);
CREATE INDEX ix_import_budget_row_category ON import_budget_row(category_id);

-- Remembered file-category → category mappings per profile (FR-IM-07, FR-IM-08). Replicated with the profile.
CREATE TABLE import_category_mapping (
    id               TEXT PRIMARY KEY,
    profile_id       TEXT NOT NULL REFERENCES import_profile(id),
    source_name_key  TEXT NOT NULL,                 -- normalize(file text), same function as category.name_key
    category_id      TEXT NOT NULL REFERENCES category(id),
    hlc INTEGER NOT NULL, hlc_device TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0,1))
) STRICT;
CREATE UNIQUE INDEX ux_import_category_mapping ON import_category_mapping(profile_id, source_name_key) WHERE deleted = 0;
CREATE INDEX ix_import_category_mapping_category ON import_category_mapping(category_id);

-- import_batch.extractor already includes 'workbook' in the 0002 target (schema.sql).

-- [D-21] Recurring amounts and [D-23] template rows moved into the 0002 target (schema.sql).
