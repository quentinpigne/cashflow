-- no-transaction
-- Famille « Bien » (ADR-0019) : trois natures de compte de plus. La contrainte CHECK de `account.kind`
-- ne se modifie pas : la table est recréée selon la procédure de SQLite (ADR-0017), contraintes de clé
-- étrangère désactivées hors transaction.
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE account_new (
    id                        TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                      TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    kind                      TEXT NOT NULL CHECK (kind IN (
        'CHECKING', 'CASH_WALLET', 'OTHER_CASH',
        'LIVRET_A', 'LDDS', 'LEP', 'LIVRET_JEUNE', 'CEL', 'PEL', 'TERM_DEPOSIT', 'SAVINGS_OTHER',
        'LIFE_INSURANCE', 'PEA', 'PEA_PME', 'PER', 'SECURITIES', 'EMPLOYEE_SAVINGS', 'CRYPTO', 'VARIABLE_OTHER',
        'MORTGAGE', 'CONSUMER_LOAN', 'LOAN_OTHER',
        'REAL_ESTATE', 'VEHICLE', 'PROPERTY_OTHER')),
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

INSERT INTO account_new (
    id, name, kind, institution_id, currency, opening_date, opening_balance, iban, account_number, closed_on,
    include_in_net_worth, include_in_emergency_fund, include_in_budget, display_order, color, notes,
    created_at, updated_at)
SELECT
    id, name, kind, institution_id, currency, opening_date, opening_balance, iban, account_number, closed_on,
    include_in_net_worth, include_in_emergency_fund, include_in_budget, display_order, color, notes,
    created_at, updated_at
FROM account;

DROP TABLE account;
ALTER TABLE account_new RENAME TO account;

COMMIT;

PRAGMA foreign_keys = ON;
