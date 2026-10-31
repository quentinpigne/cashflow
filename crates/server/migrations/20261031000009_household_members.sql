-- Membres du foyer (spec 14, ADR-0006) : des personnes, sans connexion (R-MBR-1). Le nom est unique sans
-- distinction de casse.
CREATE TABLE member (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    name          TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 40),
    color         TEXT CHECK (color IS NULL OR (length(color) = 7 AND color GLOB '#[0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f]')),
    display_order INTEGER NOT NULL,
    created_at    TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE UNIQUE INDEX member_name ON member (name COLLATE NOCASE);

-- Ce qui est affecté à un membre (R-MBR-4, R-MBR-5, R-MBR-7) ; sans membre, au foyer. La suppression d'un
-- membre passe par le service, qui remet ses affectations au foyer (R-MBR-2).
ALTER TABLE account ADD COLUMN member_id TEXT REFERENCES member (id);
ALTER TABLE split ADD COLUMN member_id TEXT REFERENCES member (id);
ALTER TABLE employment ADD COLUMN member_id TEXT REFERENCES member (id);

CREATE INDEX split_member ON split (member_id);
