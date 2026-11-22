-- Device identity from M1 (ADR-0010, DEC-2, DEC-3): the local device row is created at first
-- launch, before pairing creates its Noise key (M8) and before onboarding creates the member it
-- belongs to. Both columns become nullable; no placeholder value is ever written.
-- Table rebuild (SQLite has no ALTER COLUMN); runs with foreign keys off, checked before commit.
-- Nothing references device(id), and device stays local-only (no hlc/deleted columns).

CREATE TABLE device_new (
    id             TEXT PRIMARY KEY,
    member_id      TEXT REFERENCES member(id),     -- set by household_create / pairing
    name           TEXT NOT NULL,
    public_key     BLOB,                           -- X25519 static key, set at pairing (M8)
    is_local       INTEGER NOT NULL CHECK (is_local IN (0,1)),
    paired_at      TEXT,
    revoked_at     TEXT
) STRICT;

INSERT INTO device_new (id, member_id, name, public_key, is_local, paired_at, revoked_at)
SELECT id, member_id, name, public_key, is_local, paired_at, revoked_at FROM device;

DROP TABLE device;
ALTER TABLE device_new RENAME TO device;

CREATE UNIQUE INDEX ux_device_local ON device(is_local) WHERE is_local = 1;
CREATE UNIQUE INDEX ux_device_public_key ON device(public_key);
CREATE INDEX ix_device_member ON device(member_id);
