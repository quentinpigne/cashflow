-- Schéma initial : tout le modèle de données (docs/modele-de-donnees.md), en tables STRICT (ADR-0017).
-- Conventions : Id = TEXT de 36 caractères, Money = INTEGER (unités mineures), Rate = INTEGER (ppm),
-- Date = TEXT 'YYYY-MM-DD', Timestamp = TEXT ISO 8601 UTC, booléen = INTEGER 0/1, enum = TEXT + CHECK.

-- ===================================================================== Comptes (spec 01)

CREATE TABLE institution (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    name        TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 80),
    bank_code   TEXT CHECK (bank_code IS NULL OR length(bank_code) = 5),
    bic         TEXT CHECK (bic IS NULL OR length(bic) IN (8, 11)),
    archived    INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE account (
    id                        TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                      TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    kind                      TEXT NOT NULL CHECK (kind IN (
        'CHECKING', 'CASH_WALLET', 'OTHER_CASH',
        'LIVRET_A', 'LDDS', 'LEP', 'LIVRET_JEUNE', 'CEL', 'PEL', 'TERM_DEPOSIT', 'SAVINGS_OTHER',
        'LIFE_INSURANCE', 'PEA', 'PEA_PME', 'PER', 'SECURITIES', 'EMPLOYEE_SAVINGS', 'CRYPTO', 'VARIABLE_OTHER',
        'MORTGAGE', 'CONSUMER_LOAN', 'LOAN_OTHER')),
    institution_id            TEXT REFERENCES institution (id),
    currency                  TEXT NOT NULL DEFAULT 'EUR' CHECK (length(currency) = 3 AND currency = upper(currency)),
    opening_date              TEXT NOT NULL CHECK (opening_date = date(opening_date)),
    opening_balance           INTEGER NOT NULL DEFAULT 0,
    iban                      TEXT CHECK (iban IS NULL OR length(iban) BETWEEN 15 AND 34),
    account_number            TEXT CHECK (account_number IS NULL OR length(account_number) <= 30),
    closed_on                 TEXT CHECK (closed_on IS NULL OR (closed_on = date(closed_on) AND closed_on >= opening_date)),
    include_in_net_worth      INTEGER NOT NULL DEFAULT 1 CHECK (include_in_net_worth IN (0, 1)),
    include_in_emergency_fund INTEGER NOT NULL DEFAULT 0 CHECK (include_in_emergency_fund IN (0, 1)),
    include_in_budget         INTEGER NOT NULL DEFAULT 0 CHECK (include_in_budget IN (0, 1)),
    display_order             INTEGER NOT NULL DEFAULT 0,
    color                     TEXT,
    notes                     TEXT,
    created_at                TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at                TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE rate_schedule (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    name        TEXT NOT NULL UNIQUE CHECK (length(name) >= 1),
    shared      INTEGER NOT NULL DEFAULT 0 CHECK (shared IN (0, 1)),
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE rate_period (
    id             TEXT PRIMARY KEY CHECK (length(id) = 36),
    schedule_id    TEXT NOT NULL REFERENCES rate_schedule (id) ON DELETE CASCADE,
    effective_from TEXT NOT NULL CHECK (effective_from = date(effective_from)),
    annual_rate    INTEGER NOT NULL CHECK (annual_rate >= 0),
    created_at     TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at     TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (schedule_id, effective_from)
) STRICT;

CREATE TABLE valuation (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id  TEXT NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    date        TEXT NOT NULL CHECK (date = date(date)),
    value       INTEGER NOT NULL,
    source      TEXT NOT NULL CHECK (source IN ('MANUAL', 'IMPORT')),
    note        TEXT,
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (account_id, date)
) STRICT;

-- ===================================================================== Transactions (spec 02)

CREATE TABLE category (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    name          TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    kind          TEXT NOT NULL CHECK (kind IN ('INCOME', 'EXPENSE')),
    parent_id     TEXT REFERENCES category (id),
    archived      INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    display_order INTEGER NOT NULL DEFAULT 0,
    color         TEXT,
    icon          TEXT,
    created_at    TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

-- Nom unique au sein d'un même parent (les catégories racines ont parent_id NULL).
CREATE UNIQUE INDEX category_name_per_parent ON category (coalesce(parent_id, ''), kind, name);

CREATE TABLE fixed_savings_terms (
    account_id           TEXT PRIMARY KEY REFERENCES account (id) ON DELETE CASCADE,
    rate_schedule_id     TEXT NOT NULL REFERENCES rate_schedule (id),
    interest_method      TEXT NOT NULL CHECK (interest_method IN ('FORTNIGHTLY', 'DAILY', 'MONTHLY')),
    capitalization       TEXT NOT NULL CHECK (capitalization IN ('ANNUAL', 'MONTHLY', 'AT_MATURITY')),
    deposit_cap          INTEGER CHECK (deposit_cap IS NULL OR deposit_cap > 0),
    maturity_date        TEXT CHECK (maturity_date IS NULL OR maturity_date = date(maturity_date)),
    interest_category_id TEXT REFERENCES category (id),
    created_at           TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at           TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE recurring_rule (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id        TEXT NOT NULL REFERENCES account (id),
    target_account_id TEXT REFERENCES account (id),
    label             TEXT NOT NULL CHECK (length(label) >= 1),
    amount            INTEGER NOT NULL CHECK (amount <> 0),
    amount_tolerance  INTEGER NOT NULL DEFAULT 0 CHECK (amount_tolerance >= 0),
    frequency         TEXT NOT NULL CHECK (frequency IN ('WEEKLY', 'MONTHLY', 'QUARTERLY', 'YEARLY')),
    interval          INTEGER NOT NULL DEFAULT 1 CHECK (interval >= 1),
    anchor_date       TEXT NOT NULL CHECK (anchor_date = date(anchor_date)),
    end_date          TEXT CHECK (end_date IS NULL OR (end_date = date(end_date) AND end_date >= anchor_date)),
    split_template    TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(split_template)),
    active            INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at        TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at        TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE ledger_transaction (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id        TEXT NOT NULL REFERENCES account (id),
    date              TEXT NOT NULL CHECK (date = date(date)),
    bank_date         TEXT CHECK (bank_date IS NULL OR bank_date = date(bank_date)),
    balance_date      TEXT NOT NULL GENERATED ALWAYS AS (coalesce(bank_date, date)) STORED,
    value_date        TEXT CHECK (value_date IS NULL OR value_date = date(value_date)),
    amount            INTEGER NOT NULL CHECK (amount <> 0),
    bank_label        TEXT CHECK (bank_label IS NULL OR length(bank_label) <= 500),
    label             TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 200),
    note              TEXT,
    clearing_status   TEXT NOT NULL CHECK (clearing_status IN ('PENDING', 'CLEARED', 'RECONCILED')),
    origin            TEXT NOT NULL CHECK (origin IN ('MANUAL', 'IMPORT', 'RECURRING', 'INTEREST')),
    recurring_rule_id TEXT REFERENCES recurring_rule (id),
    fingerprint       TEXT,
    created_at        TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at        TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX ledger_transaction_account_balance_date ON ledger_transaction (account_id, balance_date);
CREATE INDEX ledger_transaction_account_date ON ledger_transaction (account_id, date);
CREATE INDEX ledger_transaction_account_status ON ledger_transaction (account_id, clearing_status);
CREATE INDEX ledger_transaction_fingerprint ON ledger_transaction (fingerprint);
CREATE UNIQUE INDEX ledger_transaction_account_fingerprint
    ON ledger_transaction (account_id, fingerprint) WHERE fingerprint IS NOT NULL;

-- ===================================================================== Budgets (spec 05), avant split

CREATE TABLE project (
    id                          TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                        TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 80),
    start_date                  TEXT NOT NULL CHECK (start_date = date(start_date)),
    end_date                    TEXT CHECK (end_date IS NULL OR (end_date = date(end_date) AND end_date >= start_date)),
    amount                      INTEGER NOT NULL CHECK (amount > 0),
    exclude_from_period_budgets INTEGER NOT NULL DEFAULT 1 CHECK (exclude_from_period_budgets IN (0, 1)),
    status                      TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'CLOSED')),
    alert_thresholds            TEXT CHECK (alert_thresholds IS NULL OR json_valid(alert_thresholds)),
    note                        TEXT,
    created_at                  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at                  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE split (
    id             TEXT PRIMARY KEY CHECK (length(id) = 36),
    transaction_id TEXT NOT NULL REFERENCES ledger_transaction (id) ON DELETE CASCADE,
    amount         INTEGER NOT NULL CHECK (amount <> 0),
    category_id    TEXT REFERENCES category (id),
    note           TEXT,
    position       INTEGER NOT NULL DEFAULT 0,
    project_id     TEXT REFERENCES project (id),
    created_at     TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at     TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX split_transaction ON split (transaction_id);
CREATE INDEX split_category ON split (category_id);
CREATE INDEX split_project ON split (project_id);

CREATE TABLE transfer (
    id                     TEXT PRIMARY KEY CHECK (length(id) = 36),
    outflow_transaction_id TEXT NOT NULL UNIQUE REFERENCES ledger_transaction (id) ON DELETE CASCADE,
    inflow_transaction_id  TEXT NOT NULL UNIQUE REFERENCES ledger_transaction (id) ON DELETE CASCADE,
    note                   TEXT,
    created_at             TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at             TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK (outflow_transaction_id <> inflow_transaction_id)
) STRICT;

-- ===================================================================== Import (spec 03)

CREATE TABLE import_profile (
    id                 TEXT PRIMARY KEY CHECK (length(id) = 36),
    name               TEXT NOT NULL UNIQUE CHECK (length(name) >= 1),
    format             TEXT NOT NULL CHECK (format IN ('CSV', 'XLSX', 'OFX', 'PDF_OCR')),
    settings           TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(settings)),
    default_account_id TEXT REFERENCES account (id),
    created_at         TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at         TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE import_batch (
    id                        TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id                TEXT NOT NULL REFERENCES account (id),
    profile_id                TEXT REFERENCES import_profile (id),
    file_name                 TEXT NOT NULL,
    file_sha256               TEXT NOT NULL CHECK (length(file_sha256) = 64),
    status                    TEXT NOT NULL CHECK (status IN ('REVIEWING', 'COMMITTED', 'CANCELLED', 'REVERTED')),
    period_start              TEXT CHECK (period_start IS NULL OR period_start = date(period_start)),
    period_end                TEXT CHECK (period_end IS NULL OR period_end = date(period_end)),
    statement_opening_balance INTEGER,
    statement_closing_balance INTEGER,
    committed_at              TEXT CHECK (committed_at IS NULL OR committed_at LIKE '%Z'),
    created_at                TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at                TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX import_batch_account_sha ON import_batch (account_id, file_sha256);

CREATE TABLE import_line (
    id                   TEXT PRIMARY KEY CHECK (length(id) = 36),
    batch_id             TEXT NOT NULL REFERENCES import_batch (id) ON DELETE CASCADE,
    line_number          INTEGER NOT NULL,
    raw                  TEXT NOT NULL CHECK (json_valid(raw)),
    date                 TEXT CHECK (date IS NULL OR date = date(date)),
    value_date           TEXT CHECK (value_date IS NULL OR value_date = date(value_date)),
    amount               INTEGER,
    bank_label           TEXT,
    fingerprint          TEXT,
    errors               TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(errors)),
    match_kind           TEXT NOT NULL DEFAULT 'NONE' CHECK (match_kind IN ('NONE', 'DUPLICATE', 'CANDIDATE')),
    match_transaction_id TEXT REFERENCES ledger_transaction (id) ON DELETE SET NULL,
    match_score          INTEGER CHECK (match_score IS NULL OR match_score BETWEEN 0 AND 100),
    suggested            TEXT CHECK (suggested IS NULL OR json_valid(suggested)),
    decision             TEXT CHECK (decision IS NULL OR decision IN ('CREATE', 'MATCH', 'SKIP')),
    transaction_id       TEXT REFERENCES ledger_transaction (id) ON DELETE SET NULL,
    created_at           TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at           TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (batch_id, line_number)
) STRICT;

CREATE INDEX import_line_transaction ON import_line (transaction_id);

CREATE TABLE categorization_rule (
    id              TEXT PRIMARY KEY CHECK (length(id) = 36),
    name            TEXT NOT NULL CHECK (length(name) >= 1),
    priority        INTEGER NOT NULL DEFAULT 0,
    conditions      TEXT NOT NULL CHECK (json_valid(conditions)),
    actions         TEXT NOT NULL CHECK (json_valid(actions)),
    stop_processing INTEGER NOT NULL DEFAULT 1 CHECK (stop_processing IN (0, 1)),
    active          INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    hit_count       INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at      TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

-- ===================================================================== Rapprochement (spec 04)

CREATE TABLE balance_checkpoint (
    id              TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id      TEXT NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    date            TEXT NOT NULL CHECK (date = date(date)),
    balance         INTEGER NOT NULL,
    source          TEXT NOT NULL CHECK (source IN ('STATEMENT', 'MANUAL')),
    import_batch_id TEXT REFERENCES import_batch (id) ON DELETE SET NULL,
    reconciled_at   TEXT CHECK (reconciled_at IS NULL OR reconciled_at LIKE '%Z'),
    created_at      TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at      TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (account_id, date)
) STRICT;

-- ===================================================================== Projections (spec 07)

CREATE TABLE projection_scenario (
    id             TEXT PRIMARY KEY CHECK (length(id) = 36),
    name           TEXT NOT NULL UNIQUE CHECK (length(name) >= 1),
    horizon_months INTEGER NOT NULL CHECK (horizon_months BETWEEN 1 AND 600),
    inflation_rate INTEGER CHECK (inflation_rate IS NULL OR inflation_rate >= 0),
    assumptions    TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(assumptions)),
    is_default     INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at     TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at     TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE UNIQUE INDEX projection_scenario_single_default ON projection_scenario (is_default) WHERE is_default = 1;

-- ===================================================================== Budgets (spec 05)

CREATE TABLE budget_line (
    id                  TEXT PRIMARY KEY CHECK (length(id) = 36),
    kind                TEXT NOT NULL CHECK (kind IN ('EXPENSE', 'INCOME', 'SAVINGS')),
    category_id         TEXT REFERENCES category (id),
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
    CHECK ((kind = 'SAVINGS') = (category_id IS NULL)),
    CHECK ((kind = 'SAVINGS') = (target_account_id IS NOT NULL)),
    CHECK (kind <> 'SAVINGS' OR periodicity = 'MONTHLY'),
    CHECK (kind <> 'INCOME' OR rollover = 0)
) STRICT;

CREATE TABLE budget_override (
    id             TEXT PRIMARY KEY CHECK (length(id) = 36),
    budget_line_id TEXT NOT NULL REFERENCES budget_line (id) ON DELETE CASCADE,
    period         TEXT NOT NULL CHECK (period GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'
                       OR period GLOB '[0-9][0-9][0-9][0-9]'),
    amount         INTEGER NOT NULL CHECK (amount >= 0),
    created_at     TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at     TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (budget_line_id, period)
) STRICT;

-- ===================================================================== Authentification (ADR-0006, ADR-0015)

CREATE TABLE app_user (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    display_name  TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 80),
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE session (
    id           TEXT PRIMARY KEY CHECK (length(id) = 36),
    user_id      TEXT NOT NULL REFERENCES app_user (id) ON DELETE CASCADE,
    token_hash   TEXT NOT NULL UNIQUE,
    device_label TEXT,
    expires_at   TEXT NOT NULL CHECK (expires_at LIKE '%Z'),
    last_seen_at TEXT NOT NULL CHECK (last_seen_at LIKE '%Z'),
    created_at   TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at   TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

-- ===================================================================== Notifications et réglages

CREATE TABLE notification (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    -- Liste ouverte (modèle de données) : validée par l'application, pas par un CHECK.
    kind        TEXT NOT NULL,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    link        TEXT,
    dedupe_key  TEXT UNIQUE,
    read_at     TEXT CHECK (read_at IS NULL OR read_at LIKE '%Z'),
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE app_setting (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL CHECK (json_valid(value)),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;
