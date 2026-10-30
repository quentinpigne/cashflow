-- Anticiper un projet (BUD-14) : mois prévu d'un poste (R-BUD-70) et début de la mise de côté (R-BUD-73).
ALTER TABLE project_line ADD COLUMN planned_month TEXT
    CHECK (planned_month IS NULL OR (planned_month GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'
                                     AND CAST(substr(planned_month, 6, 2) AS INTEGER) BETWEEN 1 AND 12));

ALTER TABLE project ADD COLUMN provision_from TEXT
    CHECK (provision_from IS NULL OR (provision_from GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]'
                                      AND CAST(substr(provision_from, 6, 2) AS INTEGER) BETWEEN 1 AND 12));
