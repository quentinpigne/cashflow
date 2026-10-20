# CLAUDE.md

Consignes pour Claude Code sur le projet Cashflow (gestion de finances personnelles).

## Phase actuelle : conception

Le dépôt ne contient que de la documentation. La stack est décidée (ADR-0009), mais on n'écrit pas de
code applicatif tant que l'utilisateur ne l'a pas demandé explicitement (fin de la phase 0 de
`docs/roadmap.md`).

Questions encore ouvertes, **à ne jamais trancher seul** (proposer, puis demander) :
`docs/questions-ouvertes.md`.

## Carte du dépôt

- `docs/README.md` : point d'entrée du dossier de conception.
- `docs/vision.md` : objectifs, non-objectifs, principes produit.
- `docs/glossaire.md` : vocabulaire métier FR ↔ identifiants EN. **Source de vérité des noms.**
- `docs/modele-de-donnees.md` : entités, champs, contraintes. **Source de vérité du schéma.**
- `docs/architecture.md` : architecture cible et sa projection sur la stack.
- `docs/specs/NN-*.md` : règles (`R-XXX-n`) et features (`XXX-nn`) par domaine.
- `docs/adr/` : décisions d'architecture.

## Règles de rédaction

- Documentation en français, identifiants de code en anglais (cf. glossaire).
- Un concept = un nom. Avant d'introduire un terme, vérifier le glossaire ; s'il manque, l'y ajouter.
- Toute modification du modèle se fait **à la fois** dans `docs/modele-de-donnees.md` et dans
  la spec du domaine concerné. Les deux ne doivent jamais diverger.
- Nouvelle décision structurante → nouvel ADR (copier `docs/adr/template.md`, numéro suivant,
  mettre à jour l'index `docs/adr/README.md`). Ne jamais réécrire le fond d'un ADR `Accepté` :
  en créer un nouveau qui le remplace (`Remplacé par ADR-XXXX`).
- Garder les identifiants `R-*` et `<DOMAINE>-nn` stables : ne jamais renuméroter, marquer
  `(supprimée)` plutôt que supprimer.

## Invariants non négociables (valables pour toute implémentation)

1. **Jamais de flottant pour l'argent.** Montants en entier de centimes (unités mineures) — ADR-0001.
2. **Le solde se calcule**, il ne se stocke pas comme source de vérité — ADR-0002.
3. **Σ ventilations = montant de la transaction**, toujours — ADR-0003.
4. **Un virement interne n'est ni un revenu ni une dépense** — ADR-0004.
5. **Un import ne modifie rien avant validation explicite**, et réimporter un même fichier
   ne crée aucun doublon — ADR-0005.
6. **Une transaction rapprochée est verrouillée** ; la modifier exige un déverrouillage explicite.
7. **Aucun secret ni aucune donnée bancaire réelle dans le dépôt** (IBAN, relevés, clés d'API).
   Les fixtures de test sont anonymisées.

## Stack (ADR-0009, ADR-0015, ADR-0016, ADR-0018 à ADR-0020)

- Backend Rust : workspace `crates/{core,app,infra,server}`, axum, sqlx + SQLite (sans ORM, ADR-0017), rust_decimal, chrono, uuid v7.
- Frontend (ADR-0019) : `web/`, Angular (standalone, signals, sans zone.js), Angular Material, Reactive Forms
  typés, ECharts. Pas de NgRx.
- Déploiement : binaire unique servant API et SPA, image Docker `linux/amd64`, HTTP simple sur le réseau local
  en V1 (ADR-0020). Un seul environnement, la production ; développement sur le Mac sans Docker (ADR-0018).
- Détails et organisation : `docs/architecture.md` § « Projection sur la stack ».

## Commandes (une fois le squelette créé)

- `just dev` : serveur et interface en développement
- `just test` : tous les tests (Rust + TypeScript)
- `just lint` : fmt, clippy `-D warnings`, `ng lint`, Prettier. **Doit passer avant tout commit.**
- `just openapi` : régénère `openapi.json` et le client TS. **À lancer après toute modification d'API.**
- `just db-prepare` : met à jour `.sqlx/`. **À lancer après toute modification de requête SQL.**

## Règles de code

- `core` : aucune I/O, aucun `f32`/`f64` (interdits par clippy), aucune dépendance à tokio, sqlx ou axum.
- Pas de `unwrap()`/`expect()` hors des tests et du démarrage du binaire. Les erreurs métier sont des
  enums `thiserror` qui portent le code de la règle `R-*` violée.
- Pas d'`unsafe` (`#![forbid(unsafe_code)]`).
- Édition Rust 2024. Modules en **fichiers homonymes** : `ledger.rs` + `ledger/split.rs`, **jamais** de
  `mod.rs` (lint `clippy::mod_module_files` en `deny` au niveau du workspace).
- Les migrations publiées sont immuables : toute évolution passe par une nouvelle migration.
- SQL : requêtes `sqlx::query!` / `query_as!` vérifiées à la compilation ; le constructeur dynamique
  (non vérifié) seulement si c'est inévitable, et couvert par un test d'intégration. Tables `STRICT`
  avec les `CHECK` du modèle de données. Une migration qui recrée une table suit la procédure de
  l'ADR-0017.
- Côté TS : ne jamais écrire à la main un type ou un service d'API (utiliser le client généré dans `web/src/app/api/`), ne jamais faire
  d'arithmétique sur des montants hors de `money.ts`.

## Quand l'implémentation commencera

- Le cœur métier (moteurs de calcul, parsing, rapprochement, projections) est pur et sans I/O
  (cf. `docs/architecture.md`) : c'est là que vont la majorité des tests.
- Chaque règle `R-*` a au moins un test qui la cite dans son nom ou sa description.
- Les messages de commit et les PR référencent les features concernées (ex. `IMP-04`).
