-- Simulations et demandes de crédit (spec 13, ADR-0027). Rien de ce qui suit n'entre dans le patrimoine, les
-- budgets ou les projections ; seul le prêt créé à partir d'une simulation est un prêt (R-CRE-7).

CREATE TABLE loan_application (
    id                     TEXT PRIMARY KEY CHECK (length(id) = 36),
    name                   TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    project_kind           TEXT NOT NULL CHECK (project_kind IN ('EXISTING_HOME', 'NEW_HOME', 'WORKS', 'VEHICLE', 'OTHER')),
    status                 TEXT NOT NULL DEFAULT 'PREPARING'
                               CHECK (status IN ('PREPARING', 'SUBMITTED', 'ACCEPTED', 'ABANDONED')),
    asset_id               TEXT REFERENCES asset (id) ON DELETE SET NULL,
    price                  INTEGER NOT NULL CHECK (price >= 0),
    notary_fees            INTEGER NOT NULL CHECK (notary_fees >= 0),
    works                  INTEGER NOT NULL CHECK (works >= 0),
    other_costs            INTEGER NOT NULL CHECK (other_costs >= 0),
    down_payment           INTEGER NOT NULL CHECK (down_payment >= 0),
    other_income           INTEGER NOT NULL DEFAULT 0 CHECK (other_income >= 0),
    other_charges          INTEGER NOT NULL DEFAULT 0 CHECK (other_charges >= 0),
    co_borrower_name       TEXT CHECK (co_borrower_name IS NULL OR length(co_borrower_name) BETWEEN 1 AND 80),
    co_borrower_income     INTEGER NOT NULL DEFAULT 0 CHECK (co_borrower_income >= 0),
    co_borrower_charges    INTEGER NOT NULL DEFAULT 0 CHECK (co_borrower_charges >= 0),
    excluded_loan_ids      TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(excluded_loan_ids)),
    -- Pas de clé étrangère : la simulation pointe déjà vers la demande ; le service tient le lien (R-CRE-23).
    selected_simulation_id TEXT CHECK (selected_simulation_id IS NULL OR length(selected_simulation_id) = 36),
    note                   TEXT,
    created_at             TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at             TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK (down_payment <= price + notary_fees + works + other_costs)
) STRICT;

CREATE TABLE loan_simulation (
    id               TEXT PRIMARY KEY CHECK (length(id) = 36),
    name             TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    kind             TEXT NOT NULL CHECK (kind IN ('MORTGAGE', 'CONSUMER', 'OTHER')),
    principal        INTEGER NOT NULL CHECK (principal > 0),
    annual_rate      INTEGER NOT NULL CHECK (annual_rate >= 0),
    duration_months  INTEGER NOT NULL CHECK (duration_months BETWEEN 1 AND 600),
    insurance_mode   TEXT NOT NULL DEFAULT 'NONE' CHECK (insurance_mode IN ('NONE', 'FIXED', 'RATE_ON_PRINCIPAL')),
    insurance_amount INTEGER CHECK (insurance_amount IS NULL OR insurance_amount >= 0),
    insurance_rate   INTEGER CHECK (insurance_rate IS NULL OR insurance_rate >= 0),
    fees             INTEGER NOT NULL DEFAULT 0 CHECK (fees >= 0),
    guarantee_fees   INTEGER NOT NULL DEFAULT 0 CHECK (guarantee_fees >= 0),
    institution_id   TEXT REFERENCES institution (id) ON DELETE SET NULL,
    application_id   TEXT REFERENCES loan_application (id) ON DELETE SET NULL,
    loan_id          TEXT UNIQUE REFERENCES loan (id) ON DELETE SET NULL,
    note             TEXT,
    created_at       TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at       TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((insurance_mode = 'FIXED') = (insurance_amount IS NOT NULL)),
    CHECK ((insurance_mode = 'RATE_ON_PRINCIPAL') = (insurance_rate IS NOT NULL))
) STRICT;

CREATE INDEX loan_simulation_application ON loan_simulation (application_id) WHERE application_id IS NOT NULL;

-- Pièce d'une demande (R-CRE-18) : un document attribué à la main, ou la pièce marquée non requise.
CREATE TABLE loan_application_piece (
    application_id TEXT NOT NULL REFERENCES loan_application (id) ON DELETE CASCADE,
    piece          TEXT NOT NULL CHECK (piece IN ('IDENTITY', 'PROOF_OF_ADDRESS', 'PAYSLIPS', 'EMPLOYMENT_CONTRACT',
                                                  'TAX_NOTICES', 'BANK_STATEMENTS', 'SAVINGS_STATEMENT',
                                                  'LOAN_DOCUMENTS', 'SALE_AGREEMENT', 'QUOTES',
                                                  'CO_BORROWER_IDENTITY', 'CO_BORROWER_PAYSLIPS',
                                                  'CO_BORROWER_CONTRACT')),
    document_id    TEXT REFERENCES document (id) ON DELETE CASCADE,
    not_required   INTEGER NOT NULL DEFAULT 0 CHECK (not_required IN (0, 1)),
    CHECK ((document_id IS NULL) = (not_required = 1))
) STRICT;

CREATE UNIQUE INDEX loan_application_piece_document ON loan_application_piece (application_id, piece, document_id)
    WHERE document_id IS NOT NULL;
CREATE UNIQUE INDEX loan_application_piece_not_required ON loan_application_piece (application_id, piece)
    WHERE not_required = 1;

-- Démarche auprès d'un établissement ou d'un courtier (R-CRE-21).
CREATE TABLE lender_request (
    id             TEXT PRIMARY KEY CHECK (length(id) = 36),
    application_id TEXT NOT NULL REFERENCES loan_application (id) ON DELETE CASCADE,
    institution_id TEXT REFERENCES institution (id) ON DELETE SET NULL,
    lender_name    TEXT CHECK (lender_name IS NULL OR length(lender_name) BETWEEN 1 AND 80),
    status         TEXT NOT NULL DEFAULT 'SENT' CHECK (status IN ('SENT', 'OFFER', 'REFUSED', 'ACCEPTED', 'WITHDRAWN')),
    sent_on        TEXT NOT NULL CHECK (sent_on = date(sent_on)),
    answered_on    TEXT CHECK (answered_on IS NULL OR (answered_on = date(answered_on) AND answered_on >= sent_on)),
    simulation_id  TEXT REFERENCES loan_simulation (id) ON DELETE SET NULL,
    note           TEXT,
    created_at     TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at     TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK (institution_id IS NOT NULL OR lender_name IS NOT NULL)
) STRICT;

CREATE INDEX lender_request_application ON lender_request (application_id);
