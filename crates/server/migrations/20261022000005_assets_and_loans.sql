-- no-transaction
-- Biens et prêts hors des comptes (ADR-0019, spec 09). Les comptes PROPERTY deviennent des biens, les comptes
-- LIABILITY des prêts (mêmes identifiants) ; le capital des mensualités ventilées devient une ventilation de
-- remboursement et la jambe sur le compte de prêt disparaît. La table des comptes est recréée selon la procédure
-- de SQLite (ADR-0017), pour retirer les deux familles de sa contrainte.
PRAGMA foreign_keys = OFF;

BEGIN;

-- Garde-fou : la migration échoue si un compte de bien ou de prêt porte une donnée qu'elle ne sait pas convertir
-- (transaction autre qu'une jambe de mensualité, récurrence, import, ligne de budget).
CREATE TEMP TABLE migration_guard (blocking INTEGER NOT NULL CHECK (blocking = 0));
INSERT INTO migration_guard
SELECT 1 WHERE EXISTS (
    SELECT 1 FROM ledger_transaction t
    JOIN account a ON a.id = t.account_id
    WHERE a.kind IN ('MORTGAGE', 'CONSUMER_LOAN', 'LOAN_OTHER', 'REAL_ESTATE', 'VEHICLE', 'PROPERTY_OTHER')
      AND t.id NOT IN (SELECT tl.inflow_transaction_id FROM transfer_leg tl
                       JOIN loan_payment lp ON lp.transaction_id = tl.outflow_transaction_id))
   OR EXISTS (
    SELECT 1 FROM account a
    WHERE a.kind IN ('MORTGAGE', 'CONSUMER_LOAN', 'LOAN_OTHER', 'REAL_ESTATE', 'VEHICLE', 'PROPERTY_OTHER')
      AND (EXISTS (SELECT 1 FROM recurring_rule r WHERE a.id IN (r.account_id, r.target_account_id))
           OR EXISTS (SELECT 1 FROM import_batch b WHERE b.account_id = a.id)
           OR EXISTS (SELECT 1 FROM budget_line l WHERE l.target_account_id = a.id)));
DROP TABLE migration_guard;

-- ===================================================================== Biens

CREATE TABLE asset (
    id                   TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                 TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    kind                 TEXT NOT NULL CHECK (kind IN ('REAL_ESTATE', 'VEHICLE', 'OTHER')),
    purchase_date        TEXT NOT NULL CHECK (purchase_date = date(purchase_date)),
    purchase_price       INTEGER NOT NULL CHECK (purchase_price > 0),
    sold_on              TEXT CHECK (sold_on IS NULL OR (sold_on = date(sold_on) AND sold_on >= purchase_date)),
    sale_price           INTEGER CHECK (sale_price IS NULL OR (sale_price >= 0 AND sold_on IS NOT NULL)),
    include_in_net_worth INTEGER NOT NULL DEFAULT 1 CHECK (include_in_net_worth IN (0, 1)),
    notes                TEXT,
    created_at           TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at           TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE asset_valuation (
    id         TEXT PRIMARY KEY CHECK (length(id) = 36),
    asset_id   TEXT NOT NULL REFERENCES asset (id) ON DELETE CASCADE,
    date       TEXT NOT NULL CHECK (date = date(date)),
    value      INTEGER NOT NULL CHECK (value > 0),
    note       TEXT,
    created_at TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (asset_id, date)
) STRICT;

INSERT INTO asset (id, name, kind, purchase_date, purchase_price, include_in_net_worth, notes, created_at, updated_at)
SELECT id, name,
       CASE kind WHEN 'REAL_ESTATE' THEN 'REAL_ESTATE' WHEN 'VEHICLE' THEN 'VEHICLE' ELSE 'OTHER' END,
       opening_date, max(opening_balance, 1), include_in_net_worth, notes, created_at, updated_at
FROM account WHERE kind IN ('REAL_ESTATE', 'VEHICLE', 'PROPERTY_OTHER');

INSERT INTO asset_valuation (id, asset_id, date, value, note, created_at, updated_at)
SELECT v.id, v.account_id, v.date, v.value, v.note, v.created_at, v.updated_at
FROM valuation v JOIN asset ON asset.id = v.account_id
WHERE v.value > 0;

-- ===================================================================== Prêts

CREATE TABLE loan (
    id                     TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                   TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    kind                   TEXT NOT NULL CHECK (kind IN ('MORTGAGE', 'CONSUMER', 'OTHER')),
    institution_id         TEXT REFERENCES institution (id),
    start_date             TEXT NOT NULL CHECK (start_date = date(start_date)),
    principal              INTEGER NOT NULL CHECK (principal > 0),
    mode                   TEXT NOT NULL CHECK (mode IN ('SCHEDULE', 'MANUAL')),
    annual_rate            INTEGER CHECK (annual_rate IS NULL OR annual_rate >= 0),
    duration_months        INTEGER CHECK (duration_months IS NULL OR duration_months BETWEEN 1 AND 600),
    first_installment_date TEXT CHECK (first_installment_date IS NULL
                                       OR (first_installment_date = date(first_installment_date)
                                           AND first_installment_date >= start_date)),
    insurance_mode         TEXT NOT NULL DEFAULT 'NONE' CHECK (insurance_mode IN ('NONE', 'FIXED', 'RATE_ON_PRINCIPAL')),
    insurance_amount       INTEGER CHECK (insurance_amount IS NULL OR insurance_amount >= 0),
    insurance_rate         INTEGER CHECK (insurance_rate IS NULL OR insurance_rate >= 0),
    payer_account_id       TEXT REFERENCES account (id),
    interest_category_id   TEXT REFERENCES category (id),
    insurance_category_id  TEXT REFERENCES category (id),
    asset_id               TEXT REFERENCES asset (id) ON DELETE SET NULL,
    closed_on              TEXT CHECK (closed_on IS NULL OR (closed_on = date(closed_on) AND closed_on >= start_date)),
    include_in_net_worth   INTEGER NOT NULL DEFAULT 1 CHECK (include_in_net_worth IN (0, 1)),
    notes                  TEXT,
    created_at             TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at             TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((mode = 'SCHEDULE') = (annual_rate IS NOT NULL AND duration_months IS NOT NULL
                                  AND first_installment_date IS NOT NULL AND payer_account_id IS NOT NULL
                                  AND interest_category_id IS NOT NULL)),
    CHECK ((insurance_mode = 'FIXED') = (insurance_amount IS NOT NULL)),
    CHECK ((insurance_mode = 'RATE_ON_PRINCIPAL') = (insurance_rate IS NOT NULL)),
    CHECK (mode = 'SCHEDULE' OR insurance_mode = 'NONE')
) STRICT;

CREATE INDEX loan_payer ON loan (payer_account_id);
CREATE INDEX loan_asset ON loan (asset_id);

CREATE TABLE loan_balance (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    loan_id     TEXT NOT NULL REFERENCES loan (id) ON DELETE CASCADE,
    date        TEXT NOT NULL CHECK (date = date(date)),
    outstanding INTEGER NOT NULL CHECK (outstanding >= 0),
    note        TEXT,
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (loan_id, date)
) STRICT;

-- Avec conditions : mode tableau ; le déblocage est daté d'un mois avant la première échéance.
INSERT INTO loan (id, name, kind, institution_id, start_date, principal, mode, annual_rate, duration_months,
                  first_installment_date, insurance_mode, insurance_amount, insurance_rate, payer_account_id,
                  interest_category_id, insurance_category_id, include_in_net_worth, notes, created_at, updated_at)
SELECT a.id, a.name,
       CASE a.kind WHEN 'MORTGAGE' THEN 'MORTGAGE' WHEN 'CONSUMER_LOAN' THEN 'CONSUMER' ELSE 'OTHER' END,
       a.institution_id, date(t.first_installment_date, '-1 month'), t.principal, 'SCHEDULE', t.annual_rate,
       t.duration_months, t.first_installment_date, t.insurance_mode, t.insurance_amount, t.insurance_rate,
       t.payer_account_id, t.interest_category_id, t.insurance_category_id, a.include_in_net_worth, a.notes,
       a.created_at, a.updated_at
FROM account a JOIN loan_terms t ON t.account_id = a.id;

-- Sans conditions : suivi simple, le capital restant dû d'ouverture tenant lieu de capital emprunté.
INSERT INTO loan (id, name, kind, institution_id, start_date, principal, mode, include_in_net_worth, notes,
                  created_at, updated_at)
SELECT a.id, a.name,
       CASE a.kind WHEN 'MORTGAGE' THEN 'MORTGAGE' WHEN 'CONSUMER_LOAN' THEN 'CONSUMER' ELSE 'OTHER' END,
       a.institution_id, a.opening_date, max(-a.opening_balance, 1), 'MANUAL', a.include_in_net_worth, a.notes,
       a.created_at, a.updated_at
FROM account a
WHERE a.kind IN ('MORTGAGE', 'CONSUMER_LOAN', 'LOAN_OTHER')
  AND NOT EXISTS (SELECT 1 FROM loan_terms t WHERE t.account_id = a.id);

INSERT INTO loan_balance (id, loan_id, date, outstanding, note, created_at, updated_at)
SELECT v.id, v.account_id, v.date, max(-v.value, 0), v.note, v.created_at, v.updated_at
FROM valuation v JOIN loan l ON l.id = v.account_id
WHERE l.mode = 'MANUAL';

-- ===================================================================== Ventilations de remboursement et d'acquisition

ALTER TABLE split ADD COLUMN loan_id TEXT REFERENCES loan (id);
ALTER TABLE split ADD COLUMN asset_id TEXT REFERENCES asset (id);
CREATE INDEX split_loan ON split (loan_id);
CREATE INDEX split_asset ON split (asset_id);

-- Le capital d'une mensualité ventilée : la ventilation de virement devient un remboursement du prêt.
UPDATE split SET loan_id = (
    SELECT lp.loan_account_id FROM loan_payment lp
    JOIN transfer_leg tl ON tl.outflow_transaction_id = lp.transaction_id
    WHERE tl.outflow_split_id = split.id)
WHERE id IN (SELECT tl.outflow_split_id FROM transfer_leg tl
             JOIN loan_payment lp ON lp.transaction_id = tl.outflow_transaction_id);

CREATE TEMP TABLE repayment_leg AS
SELECT tl.id AS transfer_id, tl.inflow_transaction_id AS leg_id
FROM transfer_leg tl JOIN loan_payment lp ON lp.transaction_id = tl.outflow_transaction_id;
DELETE FROM transfer WHERE id IN (SELECT transfer_id FROM repayment_leg);
DELETE FROM split WHERE transaction_id IN (SELECT leg_id FROM repayment_leg);
DELETE FROM ledger_transaction WHERE id IN (SELECT leg_id FROM repayment_leg);
DROP TABLE repayment_leg;

-- Échéances payées : rattachées au prêt.
CREATE TABLE loan_payment_new (
    id                 TEXT PRIMARY KEY CHECK (length(id) = 36),
    loan_id            TEXT NOT NULL REFERENCES loan (id) ON DELETE CASCADE,
    installment_number INTEGER NOT NULL CHECK (installment_number >= 1),
    transaction_id     TEXT NOT NULL UNIQUE REFERENCES ledger_transaction (id) ON DELETE CASCADE,
    created_at         TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at         TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (loan_id, installment_number)
) STRICT;
INSERT INTO loan_payment_new (id, loan_id, installment_number, transaction_id, created_at, updated_at)
SELECT id, loan_account_id, installment_number, transaction_id, created_at, updated_at FROM loan_payment;
DROP TABLE loan_payment;
ALTER TABLE loan_payment_new RENAME TO loan_payment;

DROP TABLE loan_terms;

-- ===================================================================== Comptes sans biens ni prêts

DELETE FROM valuation
WHERE account_id IN (SELECT id FROM account WHERE kind IN ('MORTGAGE', 'CONSUMER_LOAN', 'LOAN_OTHER',
                                                           'REAL_ESTATE', 'VEHICLE', 'PROPERTY_OTHER'));
DELETE FROM balance_checkpoint
WHERE account_id IN (SELECT id FROM account WHERE kind IN ('MORTGAGE', 'CONSUMER_LOAN', 'LOAN_OTHER',
                                                           'REAL_ESTATE', 'VEHICLE', 'PROPERTY_OTHER'));

CREATE TABLE account_new (
    id                        TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                      TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    kind                      TEXT NOT NULL CHECK (kind IN (
        'CHECKING', 'CASH_WALLET', 'OTHER_CASH',
        'LIVRET_A', 'LDDS', 'LEP', 'LIVRET_JEUNE', 'CEL', 'PEL', 'TERM_DEPOSIT', 'SAVINGS_OTHER',
        'LIFE_INSURANCE', 'PEA', 'PEA_PME', 'PER', 'SECURITIES', 'EMPLOYEE_SAVINGS', 'CRYPTO', 'VARIABLE_OTHER')),
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
FROM account
WHERE kind NOT IN ('MORTGAGE', 'CONSUMER_LOAN', 'LOAN_OTHER', 'REAL_ESTATE', 'VEHICLE', 'PROPERTY_OTHER');

DROP TABLE account;
ALTER TABLE account_new RENAME TO account;

-- ===================================================================== Rubriques

INSERT INTO app_setting (key, value, updated_at)
SELECT 'features.assets', 'true', strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE EXISTS (SELECT 1 FROM asset);
INSERT INTO app_setting (key, value, updated_at)
SELECT 'features.loans', 'true', strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE EXISTS (SELECT 1 FROM loan);

COMMIT;

PRAGMA foreign_keys = ON;
