-- no-transaction
-- Bibliothèque de documents (spec 12, ADR-0024) : le document devient typé et décrit (type, titre, dates, mois,
-- note), rattaché à zéro, un ou plusieurs objets (document_link), et cherchable (document_search, FTS5). Les
-- fiches de paie et les relevés de compte deviennent des documents de type PAYSLIP et BANK_STATEMENT : leurs
-- tables disparaissent. Table recréée selon la procédure de SQLite (ADR-0017) : contraintes et unicité de
-- l'empreinte (R-DOC-3).
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE document_new (
    id              TEXT PRIMARY KEY CHECK (length(id) = 36),
    kind            TEXT CHECK (kind IS NULL OR kind IN ('BANK_STATEMENT', 'PAYSLIP', 'CONTRACT', 'RECEIPT',
                                                         'INVOICE', 'TAX', 'CERTIFICATE', 'LETTER', 'OTHER')),
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

-- Un fichier archivé deux fois (même empreinte) ne garde que le document le plus ancien (R-DOC-3).
CREATE TEMP TABLE kept_document AS
SELECT d.id FROM document d
WHERE d.id = (SELECT d2.id FROM document d2 WHERE d2.sha256 = d.sha256 ORDER BY d2.created_at, d2.id LIMIT 1);

-- Titre d'un relevé ou d'une fiche : « Relevé <compte> <mois> », « Fiche de paie <employeur> <mois> » (R-DOC-15).
CREATE TEMP TABLE month_name (number TEXT PRIMARY KEY, name TEXT NOT NULL);
INSERT INTO month_name VALUES
    ('01', 'janvier'), ('02', 'février'), ('03', 'mars'), ('04', 'avril'), ('05', 'mai'), ('06', 'juin'),
    ('07', 'juillet'), ('08', 'août'), ('09', 'septembre'), ('10', 'octobre'), ('11', 'novembre'),
    ('12', 'décembre');

INSERT INTO document_new (id, kind, title, period_start, period_end, month, note, file_name, media_type,
                          size_bytes, sha256, created_at, updated_at)
SELECT d.id, 'BANK_STATEMENT',
       substr('Relevé ' || a.name || ' ' || m.name || ' ' || substr(s.statement_month, 1, 4), 1, 120),
       s.period_start, s.period_end, s.statement_month, s.note, d.file_name, d.media_type, d.size_bytes, d.sha256,
       d.created_at, s.updated_at
FROM bank_statement s
JOIN document d ON d.id = s.document_id
JOIN account a ON a.id = s.account_id
JOIN month_name m ON m.number = substr(s.statement_month, 6, 2)
WHERE d.id IN (SELECT id FROM kept_document);

INSERT INTO document_new (id, kind, title, month, note, file_name, media_type, size_bytes, sha256, created_at,
                          updated_at)
SELECT d.id, 'PAYSLIP',
       substr('Fiche de paie ' || e.employer || ' ' || m.name || ' ' || substr(p.pay_month, 1, 4), 1, 120),
       p.pay_month, p.note, d.file_name, d.media_type, d.size_bytes, d.sha256, d.created_at, p.updated_at
FROM payslip p
JOIN document d ON d.id = p.document_id
JOIN employment e ON e.id = p.employment_id
JOIN month_name m ON m.number = substr(p.pay_month, 6, 2)
WHERE d.id IN (SELECT id FROM kept_document);

-- Rattachement d'un document (R-DOC-9) : une colonne par cible, exactement une renseignée. Supprimer l'objet
-- retire le rattachement, jamais le document (R-DOC-12).
CREATE TABLE document_link (
    document_id    TEXT NOT NULL REFERENCES document (id) ON DELETE CASCADE,
    account_id     TEXT REFERENCES account (id) ON DELETE CASCADE,
    institution_id TEXT REFERENCES institution (id) ON DELETE CASCADE,
    employment_id  TEXT REFERENCES employment (id) ON DELETE CASCADE,
    transaction_id TEXT REFERENCES ledger_transaction (id) ON DELETE CASCADE,
    asset_id       TEXT REFERENCES asset (id) ON DELETE CASCADE,
    loan_id        TEXT REFERENCES loan (id) ON DELETE CASCADE,
    payee_id       TEXT REFERENCES payee (id) ON DELETE CASCADE,
    project_id     TEXT REFERENCES project (id) ON DELETE CASCADE,
    CHECK ((account_id IS NOT NULL) + (institution_id IS NOT NULL) + (employment_id IS NOT NULL)
           + (transaction_id IS NOT NULL) + (asset_id IS NOT NULL) + (loan_id IS NOT NULL)
           + (payee_id IS NOT NULL) + (project_id IS NOT NULL) = 1)
) STRICT;

INSERT INTO document_link (document_id, account_id)
SELECT document_id, account_id FROM bank_statement WHERE document_id IN (SELECT id FROM kept_document);
INSERT INTO document_link (document_id, employment_id)
SELECT document_id, employment_id FROM payslip WHERE document_id IN (SELECT id FROM kept_document);

DROP TABLE bank_statement;
DROP TABLE payslip;
DROP TABLE document;
ALTER TABLE document_new RENAME TO document;
DROP TABLE kept_document;
DROP TABLE month_name;

-- Un document est rattaché au plus une fois à un même objet ; ces index servent aussi à lister les documents d'un
-- objet.
CREATE UNIQUE INDEX document_link_account ON document_link (account_id, document_id) WHERE account_id IS NOT NULL;
CREATE UNIQUE INDEX document_link_institution ON document_link (institution_id, document_id)
    WHERE institution_id IS NOT NULL;
CREATE UNIQUE INDEX document_link_employment ON document_link (employment_id, document_id)
    WHERE employment_id IS NOT NULL;
CREATE UNIQUE INDEX document_link_transaction ON document_link (transaction_id, document_id)
    WHERE transaction_id IS NOT NULL;
CREATE UNIQUE INDEX document_link_asset ON document_link (asset_id, document_id) WHERE asset_id IS NOT NULL;
CREATE UNIQUE INDEX document_link_loan ON document_link (loan_id, document_id) WHERE loan_id IS NOT NULL;
CREATE UNIQUE INDEX document_link_payee ON document_link (payee_id, document_id) WHERE payee_id IS NOT NULL;
CREATE UNIQUE INDEX document_link_project ON document_link (project_id, document_id) WHERE project_id IS NOT NULL;
CREATE INDEX document_link_document ON document_link (document_id);

-- Index de recherche (R-DOC-18, R-DOC-19), dérivé des documents et de leurs fichiers : rempli au démarrage
-- suivant pour les documents existants (`content_indexed` à 0).
CREATE VIRTUAL TABLE document_search USING fts5(
    document_id UNINDEXED,
    title,
    note,
    file_name,
    content,
    tokenize = 'unicode61 remove_diacritics 2'
);

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
