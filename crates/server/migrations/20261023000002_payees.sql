-- Tiers et groupes de tiers (spec 11).

CREATE TABLE payee_group (
    id         TEXT PRIMARY KEY CHECK (length(id) = 36),
    name       TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(name) BETWEEN 1 AND 80),
    note       TEXT,
    created_at TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE payee (
    id         TEXT PRIMARY KEY CHECK (length(id) = 36),
    name       TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(name) BETWEEN 1 AND 80),
    -- Supprimer un groupe laisse ses tiers sans groupe (R-TIE-4).
    group_id   TEXT REFERENCES payee_group (id) ON DELETE SET NULL,
    archived   INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    note       TEXT,
    created_at TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX payee_group_id ON payee (group_id);

-- Un tiers utilisé ne se supprime pas (R-TIE-5) : pas de cascade.
ALTER TABLE ledger_transaction ADD COLUMN payee_id TEXT REFERENCES payee (id);
CREATE INDEX ledger_transaction_payee_date ON ledger_transaction (payee_id, date);

ALTER TABLE recurring_rule ADD COLUMN payee_id TEXT REFERENCES payee (id);
