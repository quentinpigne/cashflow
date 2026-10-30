-- no-transaction
-- Postes d'un projet (BUD-11, R-BUD-26 à R-BUD-28) : le prévu se détaille en postes nommés, avec des catégories
-- qui proposent le poste d'une ventilation. Un projet détaillé n'a plus de montant global : la table project est
-- recréée selon la procédure de SQLite (ADR-0017), amount figurant dans ses contraintes.
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE project_new (
    id                          TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                        TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 80),
    start_date                  TEXT NOT NULL CHECK (start_date = date(start_date)),
    end_date                    TEXT CHECK (end_date IS NULL OR (end_date = date(end_date) AND end_date >= start_date)),
    amount                      INTEGER CHECK (amount IS NULL OR amount > 0),
    exclude_from_period_budgets INTEGER NOT NULL DEFAULT 1 CHECK (exclude_from_period_budgets IN (0, 1)),
    status                      TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'CLOSED')),
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

-- Postes d'un projet (R-BUD-26).
CREATE TABLE project_line (
    id         TEXT PRIMARY KEY CHECK (length(id) = 36),
    project_id TEXT NOT NULL REFERENCES project (id) ON DELETE CASCADE,
    name       TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    amount     INTEGER NOT NULL CHECK (amount > 0),
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (project_id, id)
) STRICT;

CREATE UNIQUE INDEX project_line_name ON project_line (project_id, lower(name));

-- Catégories qui proposent un poste : une catégorie dans au plus un poste du projet (R-BUD-27).
CREATE TABLE project_line_category (
    project_line_id TEXT NOT NULL,
    project_id      TEXT NOT NULL,
    category_id     TEXT NOT NULL REFERENCES category (id),
    PRIMARY KEY (project_line_id, category_id),
    UNIQUE (project_id, category_id),
    FOREIGN KEY (project_id, project_line_id) REFERENCES project_line (project_id, id) ON DELETE CASCADE
) STRICT;

-- Poste où la ventilation est rangée (R-BUD-27) ; supprimer le poste la renvoie hors postes (R-BUD-28).
ALTER TABLE split ADD COLUMN project_line_id TEXT REFERENCES project_line (id) ON DELETE SET NULL;

CREATE INDEX split_project_line ON split (project_line_id);

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
