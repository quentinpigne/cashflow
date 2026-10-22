-- no-transaction
-- Virement porté par une ventilation (ADR-0021) : le lien relie deux ventilations au lieu de deux transactions,
-- pour qu'une mensualité de prêt porte une part de virement à côté de ses dépenses. Table recréée selon la
-- procédure de SQLite (ADR-0017). Chaque jambe existante n'a qu'une ventilation (R-TRX-32) : le virement
-- pointe désormais sur elle.
PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE transfer_new (
    id               TEXT PRIMARY KEY CHECK (length(id) = 36),
    outflow_split_id TEXT NOT NULL UNIQUE REFERENCES split (id) ON DELETE CASCADE,
    inflow_split_id  TEXT NOT NULL UNIQUE REFERENCES split (id) ON DELETE CASCADE,
    note             TEXT,
    created_at       TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at       TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK (outflow_split_id <> inflow_split_id)
) STRICT;

INSERT INTO transfer_new (id, outflow_split_id, inflow_split_id, note, created_at, updated_at)
SELECT tr.id,
       (SELECT s.id FROM split s WHERE s.transaction_id = tr.outflow_transaction_id ORDER BY s.position LIMIT 1),
       (SELECT s.id FROM split s WHERE s.transaction_id = tr.inflow_transaction_id ORDER BY s.position LIMIT 1),
       tr.note, tr.created_at, tr.updated_at
FROM transfer tr;

DROP TABLE transfer;
ALTER TABLE transfer_new RENAME TO transfer;

-- Les deux jambes d'un virement, avec leurs transactions : les requêtes au niveau de la transaction (jambe
-- opposée, candidats, compte de contrepartie) lisent cette vue.
CREATE VIEW transfer_leg AS
SELECT tr.id, tr.outflow_split_id, tr.inflow_split_id,
       so.transaction_id AS outflow_transaction_id, si.transaction_id AS inflow_transaction_id, tr.note
FROM transfer tr
JOIN split so ON so.id = tr.outflow_split_id
JOIN split si ON si.id = tr.inflow_split_id;

COMMIT;

PRAGMA foreign_keys = ON;
