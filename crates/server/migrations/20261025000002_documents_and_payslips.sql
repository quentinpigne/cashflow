-- Documents archivés sur le disque et fiches de paie (ADR-0024, spec 10, SAL-09).
-- Le contenu d'un document est le fichier `documents/<id>` du dossier des données, immuable.

CREATE TABLE document (
    id         TEXT PRIMARY KEY CHECK (length(id) = 36),
    file_name  TEXT NOT NULL CHECK (length(file_name) BETWEEN 1 AND 255),
    media_type TEXT NOT NULL CHECK (media_type IN ('application/pdf', 'image/jpeg', 'image/png')),
    size_bytes INTEGER NOT NULL CHECK (size_bytes BETWEEN 1 AND 10485760),
    sha256     TEXT NOT NULL CHECK (length(sha256) = 64),
    created_at TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

-- Le document d'une fiche est supprimé par le service, avec la fiche ou avec l'emploi (R-SAL-17).
CREATE TABLE payslip (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    employment_id TEXT NOT NULL REFERENCES employment (id) ON DELETE CASCADE,
    pay_month     TEXT NOT NULL CHECK (pay_month = date(pay_month) AND pay_month LIKE '%-01'),
    document_id   TEXT NOT NULL UNIQUE REFERENCES document (id),
    note          TEXT,
    created_at    TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX payslip_employment ON payslip (employment_id, pay_month);
