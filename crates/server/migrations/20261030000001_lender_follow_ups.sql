-- Relances et journal des démarches (spec 13, R-CRE-24 à R-CRE-26).
ALTER TABLE lender_request ADD COLUMN follow_up_on TEXT CHECK (follow_up_on IS NULL OR follow_up_on = date(follow_up_on));

CREATE TABLE lender_request_event (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    lender_request_id TEXT NOT NULL REFERENCES lender_request (id) ON DELETE CASCADE,
    date              TEXT NOT NULL CHECK (date = date(date)),
    kind              TEXT NOT NULL CHECK (kind IN ('SENT', 'FOLLOW_UP', 'CALL', 'MEETING', 'ANSWER', 'NOTE')),
    note              TEXT,
    created_at        TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at        TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX lender_request_event_request ON lender_request_event (lender_request_id, date);

-- Les démarches existantes ont leur envoi au journal.
INSERT INTO lender_request_event (id, lender_request_id, date, kind, note, created_at, updated_at)
SELECT lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-7' || substr(hex(randomblob(2)), 2) || '-'
             || substr('89ab', abs(random()) % 4 + 1, 1) || substr(hex(randomblob(2)), 2) || '-'
             || hex(randomblob(6))),
       id, sent_on, 'SENT', NULL, created_at, created_at
FROM lender_request;
