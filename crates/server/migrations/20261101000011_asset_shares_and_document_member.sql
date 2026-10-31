-- Biens répartis entre membres par quotes-parts, le reste au foyer (MBR-06, R-MBR-11) ; la suppression d'un membre
-- rend sa quote-part au foyer (R-MBR-2).
CREATE TABLE asset_share (
    asset_id  TEXT NOT NULL REFERENCES asset (id) ON DELETE CASCADE,
    member_id TEXT NOT NULL REFERENCES member (id) ON DELETE CASCADE,
    share_bp  INTEGER NOT NULL CHECK (share_bp BETWEEN 1 AND 10000),
    PRIMARY KEY (asset_id, member_id)
) STRICT;

CREATE INDEX asset_share_member ON asset_share (member_id);

-- Membre que concerne un document, ou personne (MBR-07, R-MBR-13).
ALTER TABLE document ADD COLUMN member_id TEXT REFERENCES member (id);
