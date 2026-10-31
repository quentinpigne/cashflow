-- Mois d'analyse d'une transaction (TRX-13, R-TRX-57, ADR-0028) : le mois sur lequel elle compte pour les budgets et
-- les analyses. La date d'analyse vaut la date, ou le 1er du mois d'analyse s'il diffère du mois de la date. Une
-- colonne générée ajoutée par ALTER TABLE ne peut être que VIRTUAL.
ALTER TABLE ledger_transaction ADD COLUMN analysis_month TEXT
    CHECK (analysis_month IS NULL OR (analysis_month GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'
                                      AND CAST(substr(analysis_month, 6, 2) AS INTEGER) BETWEEN 1 AND 12));

ALTER TABLE ledger_transaction ADD COLUMN analysis_date TEXT NOT NULL
    GENERATED ALWAYS AS (CASE WHEN analysis_month IS NULL OR analysis_month = substr(date, 1, 7) THEN date
                              ELSE analysis_month || '-01' END) VIRTUAL;

CREATE INDEX ledger_transaction_analysis_date ON ledger_transaction (analysis_date);
