-- Conditions de prêt (ADR-0020, R-CPT-15) : de quoi calculer le tableau d'amortissement d'un compte LIABILITY.
CREATE TABLE loan_terms (
    account_id             TEXT PRIMARY KEY REFERENCES account (id) ON DELETE CASCADE,
    principal              INTEGER NOT NULL CHECK (principal > 0),
    annual_rate            INTEGER NOT NULL CHECK (annual_rate >= 0),
    duration_months        INTEGER NOT NULL CHECK (duration_months BETWEEN 1 AND 600),
    first_installment_date TEXT NOT NULL CHECK (first_installment_date = date(first_installment_date)),
    insurance_mode         TEXT NOT NULL CHECK (insurance_mode IN ('NONE', 'FIXED', 'RATE_ON_PRINCIPAL')),
    insurance_amount       INTEGER CHECK (insurance_amount IS NULL OR insurance_amount >= 0),
    insurance_rate         INTEGER CHECK (insurance_rate IS NULL OR insurance_rate >= 0),
    payer_account_id       TEXT NOT NULL REFERENCES account (id),
    interest_category_id   TEXT NOT NULL REFERENCES category (id),
    insurance_category_id  TEXT REFERENCES category (id),
    created_at             TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at             TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((insurance_mode = 'FIXED') = (insurance_amount IS NOT NULL)),
    CHECK ((insurance_mode = 'RATE_ON_PRINCIPAL') = (insurance_rate IS NOT NULL)),
    CHECK (payer_account_id <> account_id)
) STRICT;

CREATE INDEX loan_terms_payer ON loan_terms (payer_account_id);
