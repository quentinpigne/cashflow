# ADR-0025 — Adresse des tiers et géocodage par la Base Adresse Nationale

- **Statut** : Accepté
- **Date** : 2026-10-28
- **Concerne** : spec 11, modèle de données, écrans (E15, E31, E32, E9), architecture

## Contexte

On veut analyser les dépenses **par lieu** : par ville, par département, par pays, et sur une carte. Le lieu
d'une dépense est celui du tiers (le magasin, le restaurant) : la transaction bancaire ne porte au mieux
qu'une ville tronquée dans son libellé. Placer un tiers sur une carte demande des coordonnées, que personne
ne saisit à la main : il faut un service de géocodage, donc un service tiers (ADR-0007). La Base Adresse
Nationale (BAN, `api-adresse.data.gouv.fr`) est publique, gratuite, sans clé d'API, mais ne couvre que la
France. Le domaine n'admet aucun flottant (ADR-0001).

## Options envisagées

1. **Adresse en texte libre** — ✅ simple / ❌ impossible de regrouper de façon fiable (« Meylan »,
   « 38240 MEYLAN »).
2. **Adresse structurée sur le tiers** (ligne, code postal, ville, pays) et **coordonnées facultatives** —
   ✅ regroupements fiables, carte possible / ❌ quatre champs à saisir, d'où l'intérêt d'une recherche
   d'adresse.
3. **Lieu porté par la transaction** — ✅ historise un déménagement du tiers / ❌ à saisir à chaque
   transaction ; un tiers qui change d'adresse est en pratique un autre tiers (un autre magasin).

Pour le géocodage :

1. **Recherche d'adresse par la BAN, via le serveur** : on tape l'adresse, on choisit une proposition, qui
   remplit les champs et les coordonnées. ✅ l'utilisateur valide le résultat ; le navigateur ne parle qu'au
   serveur / ❌ France seulement.
2. **Géocodage automatique à l'enregistrement** (meilleur résultat) — ❌ un résultat faux passe inaperçu.
3. **Coordonnées saisies à la main** — ❌ fastidieux, personne ne le fera.

Pour la carte :

1. **Tuiles d'un service de cartographie** (OpenStreetMap…) — ❌ un service tiers de plus, appelé par le
   navigateur à chaque affichage.
2. **Fond de carte des départements embarqué** dans l'application (GeoJSON simplifié, ECharts) — ✅ aucun
   appel externe, colorie les départements / ❌ France métropolitaine seulement, pas de rues.

## Décision

- L'**adresse est structurée et portée par le tiers** : ligne d'adresse, code postal, ville, pays (code
  ISO 3166-1 alpha-2). Elle est facultative ; dès qu'un de ses champs est renseigné, la ville et le pays sont
  requis. Elle n'est pas historisée : changer l'adresse d'un tiers change le lieu de toutes ses transactions
  dans les analyses.
- Les **coordonnées** sont deux entiers, latitude et longitude en **millionièmes de degré** (environ 10 cm),
  sans flottant. Elles accompagnent toujours une adresse et proviennent de la recherche d'adresse.
- Le **département** n'est pas stocké : il se déduit du code postal d'une adresse française (fonction pure
  du domaine). Les rares codes postaux partagés entre deux départements sont rattachés à celui de leurs deux
  premiers chiffres.
- Le **géocodage** passe par la BAN, appelée **par le serveur** (`GET /geocoding/search`) : le navigateur ne
  contacte jamais un service tiers. Conformément à l'ADR-0007, il est **désactivé par défaut**, s'active dans
  les réglages (`geocoding.enabled`), et le champ de recherche indique que le texte saisi est envoyé à la BAN.
  L'adresse de base du service est configurable (`CASHFLOW_GEOCODING_URL`) ; aucun secret n'est nécessaire.
  Désactivé, l'adresse se saisit champ par champ, sans coordonnées.
- La **carte** utilise un fond des départements de France métropolitaine **embarqué** dans le front (données
  IGN Admin Express, Licence ouverte, simplifiées) : départements coloriés selon le montant et tiers placés
  par leurs coordonnées.
- Le serveur gagne un client HTTP (`reqwest`, TLS en Rust pur, délai d'attente court). Le géocodeur n'est
  pas derrière un trait : la lecture de la réponse est une fonction pure, testée ; l'appel lui-même est
  trivial.

## Conséquences

- Analyse par pays, département, ville et carte (TIE-07), sans service tiers obligatoire.
- La recherche d'adresse ne fonctionne qu'en France ; une adresse à l'étranger se saisit à la main, sans
  coordonnées, et figure dans les analyses par pays et par ville mais pas sur la carte.
- Le serveur doit pouvoir joindre Internet pour géocoder ; sans accès, la recherche échoue proprement et la
  saisie manuelle reste possible.
- La BAN migre vers la Géoplateforme de l'IGN : si l'adresse change, il suffit de la reconfigurer.
- Le fond de carte (environ 300 Ko) est chargé à la demande, avec l'onglet Par lieu.
