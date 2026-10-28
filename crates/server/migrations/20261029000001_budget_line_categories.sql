-- no-transaction
-- Ligne de budget sur plusieurs catégories (R-BUD-10) : les catégories d'une version de ligne passent dans
-- budget_line_category, et la ligne gagne un nom (« Voiture » pour Carburant et Péage). Table recréée selon la
-- procédure de SQLite (ADR-0017) : category_id figure dans ses contraintes.
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE budget_line_new (
    id                  TEXT PRIMARY KEY CHECK (length(id) = 36),
    kind                TEXT NOT NULL CHECK (kind IN ('EXPENSE', 'INCOME', 'SAVINGS')),
    name                TEXT CHECK (name IS NULL OR length(name) BETWEEN 1 AND 60),
    target_account_id   TEXT REFERENCES account (id),
    periodicity         TEXT NOT NULL CHECK (periodicity IN ('MONTHLY', 'YEARLY')),
    year_start_month    INTEGER NOT NULL DEFAULT 1 CHECK (year_start_month BETWEEN 1 AND 12),
    amount              INTEGER NOT NULL CHECK (amount > 0),
    rollover            INTEGER NOT NULL DEFAULT 0 CHECK (rollover IN (0, 1)),
    effective_from      TEXT NOT NULL CHECK (effective_from GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'
                            AND CAST(substr(effective_from, 6, 2) AS INTEGER) BETWEEN 1 AND 12),
    effective_to        TEXT CHECK (effective_to IS NULL OR (effective_to GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'
                            AND CAST(substr(effective_to, 6, 2) AS INTEGER) BETWEEN 1 AND 12
                            AND effective_to >= effective_from)),
    alert_thresholds    TEXT CHECK (alert_thresholds IS NULL OR json_valid(alert_thresholds)),
    previous_version_id TEXT REFERENCES budget_line (id),
    created_at          TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at          TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((kind = 'SAVINGS') = (target_account_id IS NOT NULL)),
    CHECK (kind <> 'SAVINGS' OR (periodicity = 'MONTHLY' AND name IS NULL)),
    CHECK (kind <> 'INCOME' OR rollover = 0)
) STRICT;

INSERT INTO budget_line_new (id, kind, name, target_account_id, periodicity, year_start_month, amount, rollover,
                             effective_from, effective_to, alert_thresholds, previous_version_id, created_at, updated_at)
SELECT id, kind, NULL, target_account_id, periodicity, year_start_month, amount, rollover,
       effective_from, effective_to, alert_thresholds, previous_version_id, created_at, updated_at
FROM budget_line;

-- Catégories d'une version de ligne de dépense ou de revenu (au moins une, R-BUD-10).
CREATE TABLE budget_line_category (
    budget_line_id TEXT NOT NULL REFERENCES budget_line (id) ON DELETE CASCADE,
    category_id    TEXT NOT NULL REFERENCES category (id),
    PRIMARY KEY (budget_line_id, category_id)
) STRICT;

INSERT INTO budget_line_category (budget_line_id, category_id)
SELECT id, category_id FROM budget_line WHERE category_id IS NOT NULL;

DROP TABLE budget_line;
ALTER TABLE budget_line_new RENAME TO budget_line;

CREATE INDEX budget_line_category_by_category ON budget_line_category (category_id);

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
