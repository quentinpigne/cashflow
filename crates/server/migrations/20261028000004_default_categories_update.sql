-- Jeu de catégories par défaut mis à jour (R-TRX-23) : une base déjà en service reçoit les catégories qui lui
-- manquent, comme si elles avaient toujours été là ; une sous-catégorie seulement si son parent existe encore.
-- Une base neuve (aucune catégorie) n'est pas touchée : le jeu complet est créé au premier lancement.

-- Identifiant UUID (version 4) ; nouvelles catégories racines.
INSERT INTO category (id, name, kind, display_order, created_at, updated_at)
SELECT lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))), 2) || '-'
       || substr('89ab', 1 + abs(random()) % 4, 1) || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6))),
       new.name, 'EXPENSE', new.ord, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM (SELECT 'Soins et bien-être' AS name, 6 AS ord
      UNION ALL SELECT 'Éducation et formation' AS name, 9 AS ord) AS new
WHERE EXISTS (SELECT 1 FROM category)
  AND NOT EXISTS (SELECT 1 FROM category c
                  WHERE c.parent_id IS NULL AND c.kind = 'EXPENSE' AND c.name = new.name COLLATE NOCASE);

-- Nouvelles sous-catégories, sous leur parent.
INSERT INTO category (id, name, kind, parent_id, display_order, created_at, updated_at)
SELECT lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))), 2) || '-'
       || substr('89ab', 1 + abs(random()) % 4, 1) || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6))),
       new.name, 'EXPENSE', p.id, new.ord, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM (SELECT 'Alimentation & restauration' AS parent, 'Livraison de repas' AS name, 4 AS ord
      UNION ALL SELECT 'Achats & shopping' AS parent, 'High-tech / électronique' AS name, 1 AS ord
      UNION ALL SELECT 'Achats & shopping' AS parent, 'Maison et bricolage' AS name, 2 AS ord
      UNION ALL SELECT 'Achats & shopping' AS parent, 'Cadeaux' AS name, 3 AS ord
      UNION ALL SELECT 'Auto & transports' AS parent, 'Recharge électrique' AS name, 1 AS ord
      UNION ALL SELECT 'Auto & transports' AS parent, 'Stationnement' AS name, 3 AS ord
      UNION ALL SELECT 'Auto & transports' AS parent, 'Taxi / VTC' AS name, 6 AS ord
      UNION ALL SELECT 'Auto & transports' AS parent, 'Vélo et trottinette' AS name, 7 AS ord
      UNION ALL SELECT 'Abonnements' AS parent, 'Streaming (vidéo, musique)' AS name, 2 AS ord
      UNION ALL SELECT 'Abonnements' AS parent, 'Logiciels et services en ligne' AS name, 3 AS ord
      UNION ALL SELECT 'Abonnements' AS parent, 'IA' AS name, 4 AS ord
      UNION ALL SELECT 'Abonnements' AS parent, 'Presse' AS name, 5 AS ord
      UNION ALL SELECT 'Santé' AS parent, 'Dentiste' AS name, 1 AS ord
      UNION ALL SELECT 'Santé' AS parent, 'Optique' AS name, 2 AS ord
      UNION ALL SELECT 'Santé' AS parent, 'Mutuelle' AS name, 4 AS ord
      UNION ALL SELECT 'Soins et bien-être' AS parent, 'Coiffeur' AS name, 0 AS ord
      UNION ALL SELECT 'Soins et bien-être' AS parent, 'Beauté' AS name, 1 AS ord
      UNION ALL SELECT 'Soins et bien-être' AS parent, 'Autres' AS name, 2 AS ord
      UNION ALL SELECT 'Loisirs' AS parent, 'Cinéma' AS name, 1 AS ord
      UNION ALL SELECT 'Loisirs' AS parent, 'Jeux vidéo' AS name, 4 AS ord) AS new
JOIN category p ON p.parent_id IS NULL AND p.kind = 'EXPENSE' AND p.name = new.parent
WHERE NOT EXISTS (SELECT 1 FROM category c WHERE c.parent_id = p.id AND c.name = new.name COLLATE NOCASE);

-- Les catégories du jeu reprennent sa place ; les autres gardent la leur.
WITH seed (name, ord) AS (SELECT 'Logement' AS name, 0 AS ord
                 UNION ALL SELECT 'Alimentation & restauration' AS name, 1 AS ord
                 UNION ALL SELECT 'Achats & shopping' AS name, 2 AS ord
                 UNION ALL SELECT 'Auto & transports' AS name, 3 AS ord
                 UNION ALL SELECT 'Abonnements' AS name, 4 AS ord
                 UNION ALL SELECT 'Santé' AS name, 5 AS ord
                 UNION ALL SELECT 'Soins et bien-être' AS name, 6 AS ord
                 UNION ALL SELECT 'Loisirs' AS name, 7 AS ord
                 UNION ALL SELECT 'Voyages' AS name, 8 AS ord
                 UNION ALL SELECT 'Éducation et formation' AS name, 9 AS ord
                 UNION ALL SELECT 'Impôts et taxes' AS name, 10 AS ord
                 UNION ALL SELECT 'Assurances' AS name, 11 AS ord
                 UNION ALL SELECT 'Enfants' AS name, 12 AS ord
                 UNION ALL SELECT 'Animaux' AS name, 13 AS ord
                 UNION ALL SELECT 'Dons' AS name, 14 AS ord
                 UNION ALL SELECT 'Frais bancaires' AS name, 15 AS ord
                 UNION ALL SELECT 'Prêts' AS name, 16 AS ord)
UPDATE category SET display_order = (SELECT ord FROM seed WHERE seed.name = category.name)
WHERE parent_id IS NULL AND kind = 'EXPENSE' AND name IN (SELECT name FROM seed);

-- Dans les catégories complétées, « Autres » reste en dernier.
WITH seed (parent, name, ord) AS (SELECT 'Alimentation & restauration' AS parent, 'Supermarché / épicerie' AS name, 0 AS ord
                         UNION ALL SELECT 'Alimentation & restauration' AS parent, 'Restaurants' AS name, 1 AS ord
                         UNION ALL SELECT 'Alimentation & restauration' AS parent, 'Cafés / bars' AS name, 2 AS ord
                         UNION ALL SELECT 'Alimentation & restauration' AS parent, 'Fast food' AS name, 3 AS ord
                         UNION ALL SELECT 'Alimentation & restauration' AS parent, 'Livraison de repas' AS name, 4 AS ord
                         UNION ALL SELECT 'Alimentation & restauration' AS parent, 'Autres' AS name, 5 AS ord
                         UNION ALL SELECT 'Achats & shopping' AS parent, 'Vêtements / chaussures' AS name, 0 AS ord
                         UNION ALL SELECT 'Achats & shopping' AS parent, 'High-tech / électronique' AS name, 1 AS ord
                         UNION ALL SELECT 'Achats & shopping' AS parent, 'Maison et bricolage' AS name, 2 AS ord
                         UNION ALL SELECT 'Achats & shopping' AS parent, 'Cadeaux' AS name, 3 AS ord
                         UNION ALL SELECT 'Achats & shopping' AS parent, 'Autres' AS name, 4 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Carburant' AS name, 0 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Recharge électrique' AS name, 1 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Péage' AS name, 2 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Stationnement' AS name, 3 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Entretien' AS name, 4 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Transports en commun' AS name, 5 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Taxi / VTC' AS name, 6 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Vélo et trottinette' AS name, 7 AS ord
                         UNION ALL SELECT 'Auto & transports' AS parent, 'Autres' AS name, 8 AS ord
                         UNION ALL SELECT 'Abonnements' AS parent, 'Internet' AS name, 0 AS ord
                         UNION ALL SELECT 'Abonnements' AS parent, 'Téléphone' AS name, 1 AS ord
                         UNION ALL SELECT 'Abonnements' AS parent, 'Streaming (vidéo, musique)' AS name, 2 AS ord
                         UNION ALL SELECT 'Abonnements' AS parent, 'Logiciels et services en ligne' AS name, 3 AS ord
                         UNION ALL SELECT 'Abonnements' AS parent, 'IA' AS name, 4 AS ord
                         UNION ALL SELECT 'Abonnements' AS parent, 'Presse' AS name, 5 AS ord
                         UNION ALL SELECT 'Abonnements' AS parent, 'Autres' AS name, 6 AS ord
                         UNION ALL SELECT 'Santé' AS parent, 'Médecin' AS name, 0 AS ord
                         UNION ALL SELECT 'Santé' AS parent, 'Dentiste' AS name, 1 AS ord
                         UNION ALL SELECT 'Santé' AS parent, 'Optique' AS name, 2 AS ord
                         UNION ALL SELECT 'Santé' AS parent, 'Pharmacie' AS name, 3 AS ord
                         UNION ALL SELECT 'Santé' AS parent, 'Mutuelle' AS name, 4 AS ord
                         UNION ALL SELECT 'Santé' AS parent, 'Autres' AS name, 5 AS ord
                         UNION ALL SELECT 'Soins et bien-être' AS parent, 'Coiffeur' AS name, 0 AS ord
                         UNION ALL SELECT 'Soins et bien-être' AS parent, 'Beauté' AS name, 1 AS ord
                         UNION ALL SELECT 'Soins et bien-être' AS parent, 'Autres' AS name, 2 AS ord
                         UNION ALL SELECT 'Loisirs' AS parent, 'Sorties' AS name, 0 AS ord
                         UNION ALL SELECT 'Loisirs' AS parent, 'Cinéma' AS name, 1 AS ord
                         UNION ALL SELECT 'Loisirs' AS parent, 'Sport' AS name, 2 AS ord
                         UNION ALL SELECT 'Loisirs' AS parent, 'Culture' AS name, 3 AS ord
                         UNION ALL SELECT 'Loisirs' AS parent, 'Jeux vidéo' AS name, 4 AS ord
                         UNION ALL SELECT 'Loisirs' AS parent, 'Autres' AS name, 5 AS ord)
UPDATE category SET display_order = (
    SELECT seed.ord FROM seed JOIN category p ON p.id = category.parent_id
    WHERE seed.parent = p.name AND seed.name = category.name)
WHERE kind = 'EXPENSE' AND EXISTS (
    SELECT 1 FROM seed JOIN category p ON p.id = category.parent_id
    WHERE p.parent_id IS NULL AND p.kind = 'EXPENSE' AND seed.parent = p.name AND seed.name = category.name);
