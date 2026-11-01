-- no-transaction
-- Un document concerne zéro, un ou plusieurs membres (MBR-07, R-MBR-13) : la colonne document.member_id devient la
-- table document_member. Les affectations existantes sont reprises, puis la table document est recréée sans la
-- colonne selon la procédure de SQLite (ADR-0017), member_id figurant dans ses clés étrangères.
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE document_member (
    document_id TEXT NOT NULL REFERENCES document (id) ON DELETE CASCADE,
    member_id   TEXT NOT NULL REFERENCES member (id) ON DELETE CASCADE,
    PRIMARY KEY (document_id, member_id)
) STRICT;

CREATE INDEX document_member_member ON document_member (member_id);

INSERT INTO document_member (document_id, member_id)
SELECT id, member_id FROM document WHERE member_id IS NOT NULL;

CREATE TABLE document_new (
    id              TEXT PRIMARY KEY CHECK (length(id) = 36),
    kind            TEXT CHECK (kind IS NULL OR kind IN ('BANK_STATEMENT', 'PAYSLIP', 'CONTRACT', 'RECEIPT',
                                                         'INVOICE', 'TAX', 'CERTIFICATE', 'LETTER', 'DEED',
                                                         'SALE_AGREEMENT', 'VEHICLE_REGISTRATION', 'DIAGNOSTIC',
                                                         'QUOTE', 'WARRANTY', 'LOAN_OFFER',
                                                         'AMORTIZATION_SCHEDULE', 'INSURANCE', 'IDENTITY',
                                                         'PROOF_OF_ADDRESS', 'OTHER')),
    title           TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 120),
    document_date   TEXT CHECK (document_date IS NULL OR document_date = date(document_date)),
    period_start    TEXT CHECK (period_start IS NULL OR period_start = date(period_start)),
    period_end      TEXT CHECK (period_end IS NULL OR period_end = date(period_end)),
    month           TEXT CHECK (month IS NULL OR (month = date(month) AND month LIKE '%-01')),
    note            TEXT,
    file_name       TEXT NOT NULL CHECK (length(file_name) BETWEEN 1 AND 255),
    media_type      TEXT NOT NULL CHECK (media_type IN ('application/pdf', 'image/jpeg', 'image/png')),
    size_bytes      INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 10485760),
    sha256          TEXT NOT NULL UNIQUE CHECK (length(sha256) = 64),
    content_indexed INTEGER NOT NULL DEFAULT 0 CHECK (content_indexed IN (0, 1)),
    created_at      TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at      TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK (period_start IS NULL OR period_end IS NULL OR period_start <= period_end),
    -- Le mois est celui d'un relevé ou d'une fiche de paie, et seulement d'eux (R-DOC-7).
    CHECK ((coalesce(kind, '') IN ('BANK_STATEMENT', 'PAYSLIP')) = (month IS NOT NULL)),
    -- Un relevé est un PDF (R-DOC-1).
    CHECK (kind IS NOT 'BANK_STATEMENT' OR media_type = 'application/pdf')
) STRICT;

INSERT INTO document_new (id, kind, title, document_date, period_start, period_end, month, note, file_name,
                          media_type, size_bytes, sha256, content_indexed, created_at, updated_at)
SELECT id, kind, title, document_date, period_start, period_end, month, note, file_name,
       media_type, size_bytes, sha256, content_indexed, created_at, updated_at
FROM document;

DROP TABLE document;
ALTER TABLE document_new RENAME TO document;

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
