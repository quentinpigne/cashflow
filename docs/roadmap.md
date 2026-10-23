# Roadmap

Statut : **Validée** (2026-10-20)

Ordre de réalisation proposé. Chaque phase livre quelque chose d'**utilisable** et repose sur la
précédente. Les identifiants renvoient aux features des specs.

## Phase 0 — Décisions (avant toute ligne de code)

- [x] Relire et valider les ADR `Proposé` (0001 à 0008, 0010 à 0013) — 2026-10-20
- [x] Trancher la stack — ADR-0009, ADR-0015, ADR-0016, ADR-0017 (2026-10-20)
- [x] Point dédié sur les budgets — spec 05, ADR-0014 (2026-10-20)
- [x] Passer les specs de `Revue` à `Validée` — 2026-10-20
- [x] Compléter « Projection sur la stack » dans `architecture.md` et les commandes dans `CLAUDE.md`
- [x] Environnements, livraison et CI — ADR-0018 ; interface Angular, HTTP en V1, architecture en couches — ADR-0009, ADR-0013, ADR-0015 (2026-10-20)
- [x] Écrans de la phase 1 — `ecrans.md` (2026-10-20)
- [x] Passer les documents transverses à `Validée` (2026-10-20)

## Phase 1 — Socle : registre exact (MVP)

Objectif : saisir et consulter ses comptes avec des soldes exacts.

- Squelette : workspace Cargo (`domain`, `server`), `web/` (Angular), CI (ADR-0009, ADR-0013, ADR-0018)
- Authentification mono-utilisateur, `compose.yaml` en HTTP, sauvegardes automatiques (ADR-0015)
- Déploiement en une commande sur le serveur, jeu de données de démonstration (ADR-0018)
- Chaîne OpenAPI → client TypeScript (ADR-0016)
- Types `Money` / `Rate` / `Id` / `Date` avec leurs tests
- CPT-01, CPT-02, CPT-03, CPT-04 — comptes et soldes calculés
- TRX-01, TRX-02, TRX-03, TRX-04, TRX-05 — transactions, ventilation, catégories, virements
- Données initiales : établissements, catégories par défaut
- Export JSON complet (ADR-0008)
- TDB-01 (version minimale : comptes, soldes, transactions à catégoriser)

**Critère de sortie** : les tests de propriété sur Σ ventilations, solde et virements passent ; un
mois réel saisi à la main donne le bon solde.

## Phase 2 — Import et vérification (MVP)

Objectif : ne plus saisir à la main ; prouver l'exactitude.

- IMP-01 à IMP-06 — CSV/XLSX, profils, doublons, revue, pointage, validation
- RAP-01, RAP-02, RAP-03 — points de contrôle, rapprochement, verrouillage
- Golden files à partir d'exports anonymisés de ta ou tes banques

**Critère de sortie** : 3 mois de relevés réels importés, solde rapproché à 0,00 € à chaque fin de
mois, réimport sans doublon.

## Phase 3 — Confort d'import et analyse (V1)

- IMP-07 (annulation), IMP-08 (règles), IMP-09 (OFX), IMP-10 (PDF LCL), IMP-11 (virements entre lots)
- RAP-04 à RAP-07
- TRX-06 (catégorisation en masse), TRX-08 (vue transverse)
- TDB-02, TDB-03 — dépenses par catégorie, évolution, taux d'épargne

## Phase 4 — Budgets (V1)

- BUD-01 à BUD-05 — périmètre, lignes mensuelles, vue du mois, report, lignes annuelles
- BUD-06 — projets (ainsi que l'action `set_project` des règles)
- BUD-07 — objectifs d'épargne
- BUD-08, BUD-10 — alertes, centre de notifications, assistant de création
- TDB-06, TDB-07

## Phase 5 — Épargne (V1)

- CPT-05, EPA-01 à EPA-04 — taux déterminé, moteur de quinzaines, intérêts courus, plafond
- CPT-06, EPA-05, EPA-06 — valorisations, performance
- TDB-05

## Phase 6 — Projections (V1)

- TRX-07 — récurrences (nécessaires à la trésorerie et aux versements programmés)
- PRJ-01 à PRJ-05
- TDB-04

## Compléments V1 (2026-10-22)

- BIE-01 à BIE-03 — biens hors des comptes, estimations, acquisition (ADR-0019, spec 09)
- PRE-01 à PRE-04 — prêts hors des comptes, tableau d'amortissement, ventilation des mensualités
  (ADR-0019, ADR-0020, spec 09)
- TDB-08 — encadrés Biens et Prêts sur l'accueil

## Compléments V1 — salaires (2026-10-22)

- SAL-01 à SAL-05 — emplois, rémunérations et primes, évolution, comparaison au reçu, projection du salaire,
  activation (ADR-0022, spec 10)
- PRJ-10 — capacité d'épargne dans la projection du patrimoine (ADR-0022, spec 07)

## Compléments V1 — tiers (2026-10-23)

- TIE-01 à TIE-04 — tiers et groupes de tiers, affectation aux transactions, import et règles, dépenses par
  tiers et par groupe (spec 11)

## Phase 7 — V2 (à prioriser plus tard)

- IMP-12 (PDF générique OCR), IMP-13
- EPA-07, EPA-08, EPA-09
- PRJ-06 à PRJ-09
- SAL-06 à SAL-08
- BUD-09 (push mobile), BUD-11, BUD-12, BUD-13
- TRX-10, TRX-11, CPT-08, TIE-05
- Tableaux d'amortissement des prêts (hors périmètre V1, cf. vision)
- Multi-membres du foyer : accès par compte, vues par membre et par foyer (ADR-0006)

> LCL exporte en CSV/Excel la période récente, et en PDF seulement l'historique : le CSV (phase 2)
> suffit au quotidien, et IMP-10 (phase 3) sert à reprendre l'historique.
