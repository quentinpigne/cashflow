# Roadmap

Statut : **Validée** (2026-10-20) ; **V1 livrée** (2026-10-25), en attente de mise en service sur données réelles.

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

## Phase 1 — Socle : registre exact (MVP) — livrée le 2026-10-20

Objectif : saisir et consulter ses comptes avec des soldes exacts.

- [x] Squelette : workspace Cargo (`domain`, `server`), `web/` (Angular), CI (ADR-0009, ADR-0013, ADR-0018)
- [x] Authentification mono-utilisateur, `compose.yaml` en HTTP, sauvegardes automatiques (ADR-0015)
- [x] Déploiement en une commande sur le serveur, jeu de données de démonstration (ADR-0018)
- [x] Chaîne OpenAPI → client TypeScript (ADR-0016)
- [x] Types `Money` / `Rate` / `Id` / `Date` avec leurs tests
- [x] CPT-01, CPT-02, CPT-03, CPT-04 — comptes et soldes calculés
- [x] TRX-01, TRX-02, TRX-03, TRX-04, TRX-05 — transactions, ventilation, catégories, virements
- [x] Données initiales : établissements, catégories par défaut
- [x] Export JSON complet (ADR-0008)
- [x] TDB-01 (version minimale : comptes, soldes, transactions à catégoriser)

**Critère de sortie** : les tests de propriété sur Σ ventilations, solde et virements passent ✅ ; un
mois réel saisi à la main donne le bon solde (à vérifier à la mise en service).

## Phase 2 — Import et vérification (MVP) — livrée le 2026-10-21

Objectif : ne plus saisir à la main ; prouver l'exactitude.

- [x] IMP-01 à IMP-06 — CSV/XLSX, profils, doublons, revue, pointage, validation
- [x] RAP-01, RAP-02, RAP-03 — points de contrôle, rapprochement, verrouillage
- [x] Golden files à partir d'exports anonymisés de ta ou tes banques

**Critère de sortie** : 3 mois de relevés réels importés, solde rapproché à 0,00 € à chaque fin de
mois, réimport sans doublon (à vérifier à la mise en service).

## Phase 3 — Confort d'import et analyse (V1) — livrée le 2026-10-23

- [x] IMP-07 (annulation), IMP-08 (règles), IMP-09 (OFX), IMP-10 (PDF LCL), IMP-11 (virements entre lots)
- [x] RAP-04 à RAP-07
- [x] TRX-06 (catégorisation en masse), TRX-08 (vue transverse)
- [x] TDB-02, TDB-03 — dépenses par catégorie, évolution, taux d'épargne

## Phase 4 — Budgets (V1) — livrée le 2026-10-21

- [x] BUD-01 à BUD-05 — périmètre, lignes mensuelles, vue du mois, report, lignes annuelles
- [x] BUD-06 — projets (ainsi que l'action `set_project` des règles)
- [x] BUD-07 — objectifs d'épargne
- [x] BUD-08, BUD-10 — alertes, centre de notifications, assistant de création
- [x] TDB-06, TDB-07

## Phase 5 — Épargne (V1) — livrée le 2026-10-21

- [x] CPT-05, EPA-01 à EPA-04 — taux déterminé, moteur de quinzaines, intérêts courus, plafond
- [x] CPT-06, EPA-05, EPA-06 — valorisations, performance
- [x] TDB-05

## Phase 6 — Projections (V1) — livrée le 2026-10-21

- [x] TRX-07 — récurrences (nécessaires à la trésorerie et aux versements programmés)
- [x] PRJ-01 à PRJ-05
- [x] TDB-04

## Compléments V1 (2026-10-22, livrés)

- [x] BIE-01 à BIE-03 — biens hors des comptes, estimations, acquisition (ADR-0019, spec 09)
- [x] PRE-01 à PRE-04 — prêts hors des comptes, tableau d'amortissement, ventilation des mensualités
  (ADR-0019, ADR-0020, spec 09)
- [x] TDB-08 — encadrés Biens et Prêts sur l'accueil

## Compléments V1 — salaires (2026-10-22, livrés)

- [x] SAL-01 à SAL-05 — emplois, rémunérations et primes, évolution, comparaison au reçu, projection du salaire,
  activation (ADR-0022, spec 10)
- [x] PRJ-10 — capacité d'épargne dans la projection du patrimoine (ADR-0022, spec 07)

## Compléments V1 — tiers (2026-10-23, livrés)

- [x] TIE-01 à TIE-04 — tiers et groupes de tiers, affectation aux transactions, import et règles, dépenses par
  tiers et par groupe (spec 11)

## Compléments V1 — ordre des comptes et accès Foodflow (2026-10-25, livrés)

- [x] CPT-07 — ordre et couleur des comptes
- [x] Jetons d'accès en lecture seule pour Foodflow (ADR-0023 ; prérequis CSH-00 de Foodflow)

## Compléments V1 — fiches de paie (2026-10-25, livrés)

- [x] SAL-09 — archiver et consulter les fiches de paie d'un emploi (ADR-0024, spec 10)

## Compléments V1 — adresse des tiers (2026-10-28, livrés)

- [x] TIE-06 — adresse d'un tiers, recherche d'adresse par la BAN (ADR-0025, spec 11)
- [x] TIE-07 — dépenses et revenus par lieu (pays, département, ville) et carte (spec 11)

## Compléments V1 — relevés de compte (2026-10-28, livrés)

- [x] CPT-11 — archiver et consulter les relevés d'un compte, archivés aussi à l'import d'un PDF (ADR-0024,
      spec 01, R-IMP-28)

## Compléments V1 — bibliothèque de documents (2026-10-29, livrés)

- [x] Migration : `document` typé (`kind`, titre, dates, mois, note), `document_link`, reprise des tables
      `payslip` et `bank_statement`, index FTS5 `document_search` (ADR-0024, spec 12)
- [x] CPT-11, SAL-09, R-IMP-28 portés sur la bibliothèque (relevés et fiches de paie = documents typés)
- [x] DOC-01 — bibliothèque : envoi multiple, documents à classer, fiche d'un document (E34, E35)
- [x] DOC-02 — rattachements multiples, section Documents des fiches, confirmations de suppression (R-DOC-12)
- [x] DOC-03 — recherche plein texte (description, rattachements, texte des PDF), filtres,
      `cashflow documents reindex`
- [x] DOC-04 — justificatifs d'une transaction, photo depuis le téléphone, filtre en E14 (remplace TRX-11)

## Compléments V1 — crédit (2026-10-29, livrés)

- [x] Types de documents des biens, des prêts et des demandes de crédit (spec 12, ADR-0027)
- [x] CRE-01 à CRE-03 — simulations : calcul, TAEG, calculs inverses, comparaison, transformation en prêt
- [x] CRE-04 — demande de crédit : plan de financement, capacité d'emprunt, co-emprunteur
- [x] CRE-05 — pièces à fournir reliées à la bibliothèque, export ZIP et synthèse imprimable
- [x] CRE-06 — démarches auprès des banques et courtiers, offres comparées, acceptation
- [x] CRE-07 — capacité d'achat : prix maximal aujourd'hui et dans les années à venir, effet sur l'épargne et le
      patrimoine projeté, création d'une demande
- [x] CRE-08 — onglet Simuler : capacité et simulateur réunis ; préparer une demande depuis une simulation
- [x] CRE-09 — journal des démarches, relances et rappels
- [x] CRE-10 — acceptation d'une offre : bien, prêt et documents en une fois

## Compléments V1 — onglet Simuler par projet (2026-10-30, livrés)

- [x] CRE-11 — onglet Simuler par projet : situation d'emprunteur, deux questions (que puis-je acheter,
      financer un achat au prix connu), un seul jeu de conditions
- [x] CRE-12 — conditions de crédit par défaut de chaque type de projet, dans les réglages (E42)

## Compléments V1 — postes des projets (2026-10-30, livrés)

- [x] BUD-11 — postes d'un projet : prévu détaillé, catégories facultatives, réel par poste et hors postes
      (spec 05, R-BUD-26 à R-BUD-29)

## Mise en service (prochaine étape)

- [ ] Exposition sur Internet derrière un proxy, en HTTPS : passe d'exploitation et de sécurité (Q15, à
      concevoir ; remet en cause « réseau local uniquement » d'ADR-0015)
- [ ] Déploiement sur le serveur et premières données réelles. Dès lors, un ADR `Accepté` ne se modifie
      plus sur le fond (CLAUDE.md)
- [ ] Critères de sortie des phases 1 et 2 vérifiés sur données réelles

## Phase 7 — V2 (à prioriser plus tard)

- IMP-12 (PDF générique OCR), IMP-13
- EPA-07, EPA-08, EPA-09
- PRJ-06 à PRJ-09
- SAL-06 à SAL-08
- BUD-09 (push mobile), BUD-12, BUD-13
- TRX-10, CPT-08, TIE-05
- DOC-05 (échéances des contrats et garanties), DOC-06 (OCR des images, ADR-0007)
- Tableaux d'amortissement des prêts (hors périmètre V1, cf. vision)
- Multi-membres du foyer : accès par compte, vues par membre et par foyer (ADR-0006)

> LCL exporte en CSV/Excel la période récente, et en PDF seulement l'historique : le CSV (phase 2)
> suffit au quotidien, et IMP-10 (phase 3) sert à reprendre l'historique.
