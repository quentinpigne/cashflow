-- no-transaction
-- Adresse d'un tiers (spec 11, R-TIE-23 à R-TIE-25, ADR-0025). Table recréée selon la procédure de SQLite
-- (ADR-0017) pour poser les contraintes croisées : ville et pays ensemble, rien sans ville, les deux
-- coordonnées ensemble.
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE payee_new (
    id           TEXT PRIMARY KEY CHECK (length(id) = 36),
    name         TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(name) BETWEEN 1 AND 80),
    -- Supprimer un groupe laisse ses tiers sans groupe (R-TIE-4).
    group_id     TEXT REFERENCES payee_group (id) ON DELETE SET NULL,
    archived     INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    note         TEXT,
    address_line TEXT CHECK (length(address_line) BETWEEN 1 AND 200),
    postal_code  TEXT CHECK (length(postal_code) BETWEEN 1 AND 10),
    city         TEXT CHECK (length(city) BETWEEN 1 AND 80),
    country_code TEXT CHECK (length(country_code) = 2 AND country_code = upper(country_code)),
    -- Millionièmes de degré, jamais de flottant (R-TIE-25).
    latitude_e6  INTEGER CHECK (latitude_e6 BETWEEN -90000000 AND 90000000),
    longitude_e6 INTEGER CHECK (longitude_e6 BETWEEN -180000000 AND 180000000),
    created_at   TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at   TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((city IS NULL) = (country_code IS NULL)),
    CHECK (city IS NOT NULL OR (address_line IS NULL AND postal_code IS NULL AND latitude_e6 IS NULL)),
    CHECK ((latitude_e6 IS NULL) = (longitude_e6 IS NULL))
) STRICT;

INSERT INTO payee_new (id, name, group_id, archived, note, created_at, updated_at)
SELECT id, name, group_id, archived, note, created_at, updated_at FROM payee;

DROP TABLE payee;
ALTER TABLE payee_new RENAME TO payee;

CREATE INDEX payee_group_id ON payee (group_id);

COMMIT;

PRAGMA foreign_keys = ON;
