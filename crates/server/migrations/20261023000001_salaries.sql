-- Salaires : emplois, rémunérations et primes déclarées (ADR-0022, spec 10).

CREATE TABLE employment (
    id                 TEXT PRIMARY KEY CHECK (length(id) = 36),
    employer           TEXT NOT NULL CHECK (length(employer) BETWEEN 1 AND 80),
    job_title          TEXT CHECK (job_title IS NULL OR length(job_title) BETWEEN 1 AND 80),
    start_date         TEXT NOT NULL CHECK (start_date = date(start_date)),
    end_date           TEXT CHECK (end_date IS NULL OR (end_date = date(end_date) AND end_date >= start_date)),
    income_category_id TEXT NOT NULL REFERENCES category (id),
    raise_month        INTEGER NOT NULL DEFAULT 1 CHECK (raise_month BETWEEN 1 AND 12),
    paid_next_month    INTEGER NOT NULL DEFAULT 0 CHECK (paid_next_month IN (0, 1)),
    notes              TEXT,
    created_at         TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at         TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX employment_income_category ON employment (income_category_id);

CREATE TABLE salary (
    id              TEXT PRIMARY KEY CHECK (length(id) = 36),
    employment_id   TEXT NOT NULL REFERENCES employment (id) ON DELETE CASCADE,
    effective_month TEXT NOT NULL CHECK (effective_month = date(effective_month) AND effective_month LIKE '%-01'),
    annual_gross    INTEGER NOT NULL CHECK (annual_gross > 0),
    monthly_net     INTEGER NOT NULL CHECK (monthly_net > 0),
    reason          TEXT NOT NULL CHECK (reason IN ('HIRE', 'RAISE', 'PROMOTION', 'TAX_RATE', 'OTHER')),
    note            TEXT,
    created_at      TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at      TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (employment_id, effective_month)
) STRICT;

CREATE TABLE salary_bonus (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    employment_id TEXT NOT NULL REFERENCES employment (id) ON DELETE CASCADE,
    label         TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 80),
    kind          TEXT NOT NULL CHECK (kind IN ('THIRTEENTH_MONTH', 'BONUS', 'PROFIT_SHARING', 'OTHER')),
    paid_on       TEXT NOT NULL CHECK (paid_on = date(paid_on)),
    net_amount    INTEGER NOT NULL CHECK (net_amount > 0),
    gross_amount  INTEGER CHECK (gross_amount IS NULL OR gross_amount > 0),
    recurring     INTEGER NOT NULL CHECK (recurring IN (0, 1)),
    note          TEXT,
    created_at    TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX salary_bonus_employment ON salary_bonus (employment_id, paid_on);

-- Hausse des salaires des scénarios fournis par défaut : 1 % (prudent), 3 % (optimiste) ; les autres scénarios
-- prennent 2 % pour les salaires et les dépenses (R-SAL-12, R-PRJ-44).
UPDATE projection_scenario
SET assumptions = json_set(assumptions, '$.capacity',
                           json_object('enabled', json('true'), 'salary_growth_ppm', 10000, 'expense_growth_ppm', 20000))
WHERE name = 'Prudent' AND json_extract(assumptions, '$.capacity') IS NULL;

UPDATE projection_scenario
SET assumptions = json_set(assumptions, '$.capacity',
                           json_object('enabled', json('true'), 'salary_growth_ppm', 30000, 'expense_growth_ppm', 20000))
WHERE name = 'Optimiste' AND json_extract(assumptions, '$.capacity') IS NULL;
