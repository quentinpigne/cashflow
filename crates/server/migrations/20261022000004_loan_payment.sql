-- Échéances de prêt payées (R-CPT-17) : la transaction du compte payeur ventilée selon l'échéance. Une échéance
-- n'est payée qu'une fois, une transaction ne paie qu'une échéance.
CREATE TABLE loan_payment (
    id                 TEXT PRIMARY KEY CHECK (length(id) = 36),
    loan_account_id    TEXT NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    installment_number INTEGER NOT NULL CHECK (installment_number >= 1),
    transaction_id     TEXT NOT NULL UNIQUE REFERENCES ledger_transaction (id) ON DELETE CASCADE,
    created_at         TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at         TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (loan_account_id, installment_number)
) STRICT;
