-- Catégorie hors analyses (R-TRX-24) : ses ventilations ne sont ni revenu ni dépense, comme un virement.
ALTER TABLE category
    ADD COLUMN excluded_from_analyses INTEGER NOT NULL DEFAULT 0 CHECK (excluded_from_analyses IN (0, 1));

-- Catégories hors analyses, réglées ainsi ou sous un parent qui l'est : les requêtes de flux lisent cette vue.
CREATE VIEW excluded_category AS
SELECT c.id
FROM category c
LEFT JOIN category p ON p.id = c.parent_id
WHERE c.excluded_from_analyses = 1 OR p.excluded_from_analyses = 1;
