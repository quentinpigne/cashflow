-- Remboursements de frais (TRX-12, ADR-0026) : une ventilation positive rembourse une dépense, et une dépense
-- porte le remboursement qu'on en attend. Supprimer la dépense retire le lien (R-TRX-55).
ALTER TABLE split
    ADD COLUMN refunds_transaction_id TEXT REFERENCES ledger_transaction (id) ON DELETE SET NULL
        CHECK (refunds_transaction_id IS NULL
               OR (amount > 0 AND loan_id IS NULL AND asset_id IS NULL AND refunds_transaction_id <> transaction_id));

CREATE INDEX split_refunds ON split (refunds_transaction_id) WHERE refunds_transaction_id IS NOT NULL;

ALTER TABLE ledger_transaction
    ADD COLUMN expected_refund INTEGER CHECK (expected_refund IS NULL OR (expected_refund > 0 AND expected_refund <= -amount));
