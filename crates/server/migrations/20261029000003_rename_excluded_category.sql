-- La catégorie hors analyses fournie de base s'appelle « Sans catégorie » (R-TRX-25) : son nom dit l'intention,
-- « hors analyses » n'en est que l'effet. Renommée seulement si l'utilisateur ne l'a pas déjà renommée et si le
-- nouveau nom est libre.
UPDATE category
SET name = 'Sans catégorie', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE name = 'Hors analyses' AND kind = 'EXPENSE' AND parent_id IS NULL AND excluded_from_analyses = 1
  AND NOT EXISTS (SELECT 1 FROM category c
                  WHERE c.parent_id IS NULL AND c.kind = 'EXPENSE' AND c.name = 'Sans catégorie' COLLATE NOCASE);
