-- Relevés de compte archivés (CPT-11, R-CPT-18 à R-CPT-21, ADR-0024). Le document d'un relevé est supprimé par
-- le service, avec le relevé ou avec le compte (R-CPT-20).
CREATE TABLE bank_statement (
    id              TEXT PRIMARY KEY CHECK (length(id) = 36),
    account_id      TEXT NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    statement_month TEXT NOT NULL CHECK (statement_month = date(statement_month) AND statement_month LIKE '%-01'),
    period_start    TEXT CHECK (period_start IS NULL OR period_start = date(period_start)),
    period_end      TEXT CHECK (period_end IS NULL OR period_end = date(period_end)),
    document_id     TEXT NOT NULL UNIQUE REFERENCES document (id),
    note            TEXT,
    created_at      TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at      TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((period_start IS NULL) = (period_end IS NULL)),
    CHECK (period_start IS NULL OR period_start <= period_end)
) STRICT;

CREATE INDEX bank_statement_account ON bank_statement (account_id, statement_month);
