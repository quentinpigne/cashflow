-- no-transaction
-- Statut « en projet » d'un projet (R-BUD-20) : PLANNED rejoint ACTIVE et CLOSED. La table project est recréée selon
-- la procédure de SQLite (ADR-0017), status figurant dans ses contraintes.
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE project_new (
    id                          TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                        TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 80),
    start_date                  TEXT NOT NULL CHECK (start_date = date(start_date)),
    end_date                    TEXT CHECK (end_date IS NULL OR (end_date = date(end_date) AND end_date >= start_date)),
    amount                      INTEGER CHECK (amount IS NULL OR amount > 0),
    exclude_from_period_budgets INTEGER NOT NULL DEFAULT 1 CHECK (exclude_from_period_budgets IN (0, 1)),
    status                      TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('PLANNED', 'ACTIVE', 'CLOSED')),
    alert_thresholds            TEXT CHECK (alert_thresholds IS NULL OR json_valid(alert_thresholds)),
    note                        TEXT,
    created_at                  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at                  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

INSERT INTO project_new (id, name, start_date, end_date, amount, exclude_from_period_budgets, status, alert_thresholds,
                         note, created_at, updated_at)
SELECT id, name, start_date, end_date, amount, exclude_from_period_budgets, status, alert_thresholds,
       note, created_at, updated_at
FROM project;

DROP TABLE project;
ALTER TABLE project_new RENAME TO project;

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
