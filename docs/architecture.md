# Architecture cible

Statut : **Revue** — principes indépendants de la stack, projection sur Rust + Angular en fin de document.

## Vue d'ensemble

Monolithe modulaire, architecture hexagonale (ADR-0013). Une seule base SQLite à fichier unique.
Une seule API (REST + OpenAPI, ADR-0016) consommée par une SPA Angular servie par le même binaire.

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Adaptateurs entrants : API REST (SPA web), sous-commandes CLI (export,   │
│                        sauvegarde, migrations)                           │
└───────────────┬─────────────────────────────────────────────────────────┘
                │ appelle
┌───────────────▼─────────────────────────────────────────────────────────┐
│ Cas d'usage (application)                                                │
│  CreateTransaction · SplitTransaction · LinkTransfer · StartImport       │
│  ReviewImportLine · CommitImport · RevertImport · AddCheckpoint          │
│  Reconcile · Unlock · ComputeInterest · RunProjection · ExportAll …      │
│  → une transaction de base de données par cas d'usage                    │
└───────┬───────────────────────────────────────────────┬─────────────────┘
        │ utilise                                        │ via des ports
┌───────▼──────────────────────────────┐   ┌─────────────▼───────────────┐
│ Cœur métier (pur, sans I/O)           │   │ Ports (interfaces)          │
│  Types : Money, Rate, Ids, Date       │   │  Repositories (par agrégat) │
│  Entités + invariants                 │   │  StatementReader            │
│  Moteurs :                            │   │  PdfTextExtractor · OCR     │
│   · BalanceEngine (soldes à date)     │   │  Clock                      │
│   · SplitAllocator (% → montants)     │   │  Notifier (alertes budget)  │
│   · Normalizer (montants, dates)      │   │  FileStore (fichiers source)│
│   · Fingerprinter                     │   └─────────────┬───────────────┘
│   · Matcher (correspondances)         │                 │ implémentés par
│   · RuleEngine (catégorisation)       │   ┌─────────────▼───────────────┐
│   · ReconciliationEngine (écarts)     │   │ Adaptateurs sortants        │
│   · InterestEngine (quinzaines…)      │   │  SQLite · lecteurs CSV/XLSX/│
│   · PerformanceEngine (TRI, TWR)      │   │  OFX/PDF-LCL · OCR (opt-in) │
│   · ProjectionEngine                  │   │  · horloge · disque · notif.│
│   · RecurrenceExpander                │   └─────────────────────────────┘
│   · BudgetEngine · AlertEvaluator     │
│   · Indicators (tableau de bord)      │
└───────────────────────────────────────┘
```

## Modules du cœur (contextes)

| Module | Contenu | Spec |
|---|---|---|
| `accounts` | Account, Institution, familles et natures, FixedSavingsTerms, Valuation | 01 |
| `ledger` | Transaction, Split, Category, Transfer, RecurringRule, BalanceEngine, SplitAllocator, RecurrenceExpander | 02 |
| `import` | ImportProfile, ImportBatch, ImportLine, Normalizer, Fingerprinter, Matcher, RuleEngine | 03 |
| `reconciliation` | BalanceCheckpoint, ReconciliationEngine | 04 |
| `budgeting` | BudgetLine, BudgetOverride, Project, BudgetEngine (résolution des lignes, reports, quotes-parts), AlertEvaluator | 05 |
| `savings` | RateSchedule, InterestEngine, PerformanceEngine | 06 |
| `projection` | ProjectionScenario, ProjectionEngine | 07 |
| `reporting` | Indicators | 08 |
| `shared` | Money, Rate, Ids, Date, erreurs métier | — |

Dépendances autorisées : `shared` ← tous ; `ledger` ← `import`, `reconciliation`, `budgeting`,
`savings`, `projection`, `reporting` ; `accounts` ← tous sauf `shared`. Pas de cycle. `import` ne
dépend pas de `reconciliation` : la création des points de contrôle à la validation passe par le cas
d'usage `CommitImport`.

## Flux clés

### Import (spec 03)

```
StartImport(fichier, compte, profil)
  → StatementReader.read(fichier, profil)     [adaptateur : CSV/XLSX/OFX/PDF]
  → Normalizer → Fingerprinter                 [cœur]
  → Matcher(lignes, transactions candidates)   [cœur ; candidats chargés par le cas d'usage]
  → RuleEngine(lignes, règles)                 [cœur]
  → persistance ImportBatch + ImportLine (REVIEWING)
ReviewImportLine(...)                          → mise à jour d'une décision
CommitImport(batch)                            → 1 transaction BDD : transactions, pointages,
                                                 virements, points de contrôle, statut COMMITTED
```

### Solde et rapprochement (spec 04)

`BalanceEngine` reçoit le solde d'ouverture et les montants datés, et produit le solde à une date ou
une série. Les agrégats SQL (Σ par compte jusqu'à D) sont une optimisation de l'adaptateur de
persistance : ils doivent produire exactement le même résultat que le moteur, ce que vérifie un test
de cohérence.

## Gestion des erreurs

- Les violations de règles métier sont des **erreurs typées** du cœur (ex. `SplitSumMismatch`,
  `TransactionLocked{checkpoint_id}`, `AccountClosed`), jamais des chaînes ni des exceptions génériques.
  L'interface les traduit en messages ; chacune cite la règle `R-*` concernée.
- Les erreurs de lecture de fichier sont des **données** (champ `errors` de `ImportLine`), pas des
  exceptions (R-IMP-3).

## Stratégie de test

| Niveau | Cible | Outils |
|---|---|---|
| Unitaire | Moteurs du cœur, règle par règle (`R-*`) | `cargo test` ; Vitest (`ng test`) pour `money.ts` et la logique d'interface |
| Propriétés | Invariants : Σ ventilations, solde = ouverture + Σ, réimport idempotent, patrimoine invariant par virement, annulation d'import = état initial | `proptest` |
| Golden files | Lecteurs de fichiers : `fixtures/<banque>/<cas>.csv` → `expected.json` | `insta`, fichiers **anonymisés** uniquement |
| Intégration | Cas d'usage avec une SQLite réelle (fichier temporaire ou mémoire) | — |
| Aller-retour | Export JSON → import → export identique | — |
| Bout en bout | Parcours critiques : importer, revoir, valider, rapprocher | Playwright |

Les cas de référence chiffrés des specs (intérêts, TRI, ventilations, indicateurs) sont repris
**tels quels** comme tests.

## Sécurité et confidentialité

- Aucune donnée réelle ni aucun secret dans le dépôt (cf. CLAUDE.md). `.gitignore` initial : base de
  données, fichiers importés, `.env*` sauf `.env.example`.
- Accès : réseau local uniquement, mot de passe unique et session par cookie (ADR-0015). HTTP simple en
  V1, HTTPS servi par axum plus tard (ADR-0020).
- Chiffrement au repos : chiffrement du disque du serveur (ADR-0015).
- Journalisation : jamais de libellé, de montant ou d'IBAN dans les logs au niveau INFO.
- OCR : ADR-0007.

## Migrations de schéma

- Migrations versionnées, uniquement en avant, jamais modifiées une fois publiées.
- Sauvegarde automatique de la base avant chaque migration (ADR-0008).
- Les **données initiales** (établissements, catégories par défaut, barèmes réglementés, scénarios)
  sont chargées par un mécanisme de seed **distinct** des migrations de schéma, idempotent, sans
  aucune donnée personnelle.

## Projection sur la stack

Stack : ADR-0009 (Rust), ADR-0019 (Angular). Déploiement : ADR-0015, ADR-0018, ADR-0020. API : ADR-0016.

### Organisation du dépôt

```
cashflow/
├── Cargo.toml                 ← workspace Rust
├── crates/
│   ├── core/                  ← cœur métier pur (ADR-0013)
│   │   └── src/{shared,accounts,ledger,import,reconciliation,
│   │            budgeting,savings,projection,reporting}.rs + dossiers homonymes
│   ├── app/                   ← cas d'usage + définitions des ports (traits)
│   ├── infra/                 ← adaptateurs : sqlx/SQLite, lecteurs CSV/XLSX/OFX/PDF, OCR, Notifier
│   │   ├── migrations/        ← migrations sqlx (immuables une fois publiées)
│   │   └── seed/              ← données initiales (JSON), sans donnée personnelle
│   └── server/                ← axum, DTO, OpenAPI (utoipa), auth, fichiers statiques,
│                                 binaire `cashflow` et ses sous-commandes
├── web/                       ← SPA Angular (Angular CLI)
│   └── src/app/api/           ← client généré depuis openapi.json (jamais édité à la main)
├── openapi.json               ← contrat d'API versionné (ADR-0016)
├── fixtures/                  ← fichiers d'import anonymisés + résultats attendus (golden)
├── deploy/                    ← Dockerfile, compose.yaml, guide d'installation
├── docs/                    ← conception : docs transverses, specs/, adr/
├── justfile                   ← commandes du projet
├── README.md · CLAUDE.md
```

### Conventions Rust

- Édition **2024** pour tous les crates (`edition = "2024"` dans `[workspace.package]`).
- Modules en **fichiers homonymes**, sans `mod.rs` :
  ```
  crates/core/src/
  ├── lib.rs
  ├── ledger.rs            ← déclare `pub mod split; pub mod transfer; …`
  └── ledger/
      ├── split.rs
      └── transfer.rs
  ```
- Lints communs dans le `Cargo.toml` du workspace, hérités par chaque crate (`[lints] workspace = true`) :
  ```toml
  [workspace.lints.rust]
  unsafe_code = "forbid"

  [workspace.lints.clippy]
  mod_module_files = "deny"   # interdit les mod.rs
  unwrap_used = "deny"        # autorisés dans les tests (allow-unwrap-in-tests / allow-expect-in-tests
  expect_used = "deny"        # dans clippy.toml) et par un #[allow] ciblé au démarrage du binaire
  ```
  Les flottants sont interdits dans `core` par `disallowed-types`, dans `crates/core/clippy.toml`. Ce
  fichier remplace celui de la racine pour ce crate (clippy ne fusionne pas les deux) : il doit donc
  reprendre aussi les options `allow-*-in-tests`.

### Dépendances entre crates

```
server ──► app ──► core
   │        ▲
   └──► infra ┘   (infra implémente les ports définis dans app ; server assemble le tout)
```

- `core` ne dépend d'aucun crate d'I/O (ni `tokio`, ni `sqlx`, ni `axum`, ni `reqwest`). Ses dépendances
  autorisées : `rust_decimal`, `chrono`, `uuid`, `serde`, `thiserror`. Les flottants y sont interdits
  par clippy (`disallowed-types`).
- `app` définit les ports sous forme de traits (`AccountRepository`, `StatementReader`, `Clock`,
  `Notifier`…) et les cas d'usage, génériques sur ces traits. Chaque cas d'usage s'exécute dans une
  **unité de travail** (`UnitOfWork`), qui correspond à une transaction SQLite.
- `infra` et `server` sont les seuls crates à connaître `sqlx`, respectivement `axum`.

### Correspondance des modules

| Module du cœur | Moteurs (dans `core`) | Adaptateurs (`infra`) | Routes (`server`, `/api/v1`) |
|---|---|---|---|
| `accounts` | — | `SqliteAccountRepository` | `/institutions`, `/accounts`, `/accounts/{id}/valuations` |
| `ledger` | `BalanceEngine`, `SplitAllocator`, `RecurrenceExpander` | `SqliteLedgerRepository` | `/transactions`, `/categories`, `/transfers`, `/recurring-rules` |
| `import` | `Normalizer`, `Fingerprinter`, `Matcher`, `RuleEngine` | `CsvReader`, `XlsxReader`, `OfxReader`, `LclPdfReader`, `MistralOcr` | `/import-profiles`, `/imports`, `/imports/{id}/lines`, `/rules` |
| `reconciliation` | `ReconciliationEngine` | `SqliteCheckpointRepository` | `/accounts/{id}/checkpoints` |
| `budgeting` | `BudgetEngine`, `AlertEvaluator` | `SqliteBudgetRepository`, `DbNotifier` | `/budget/lines`, `/budget/months/{yyyy-mm}`, `/projects`, `/notifications` |
| `savings` | `InterestEngine`, `PerformanceEngine` | `SqliteRateRepository` | `/rate-schedules`, `/accounts/{id}/interest`, `/accounts/{id}/performance` |
| `projection` | `ProjectionEngine` | `SqliteScenarioRepository` | `/scenarios`, `/projections` |
| `reporting` | `Indicators` | — | `/dashboard`, `/reports/*` |
| — | — | export JSON | `/export` (et sous-commande `cashflow export`) |

### SQLite

Décision et précautions : ADR-0017.

- SQLite embarqué (≥ 3.37), tables `STRICT`, `journal_mode = WAL`, `foreign_keys = ON`,
  `synchronous = NORMAL`, `busy_timeout = 5000` ; une seule connexion en écriture.
- Types physiques : cf. `modele-de-donnees.md` § « Correspondance physique SQLite ».
- L'invariant Σ ventilations est garanti par le cœur **et** contrôlé en fin d'unité de travail, en
  production comme en test.
- Migrations : fichiers SQL `sqlx migrate`, transactionnels. Les migrations qui recréent une table
  suivent la procédure de l'ADR-0017 (point 4).
- Tests d'intégration : une base SQLite en mémoire (ou un fichier temporaire) par test, migrations
  appliquées, en parallèle.

### Interface (`web/`)

- Pages : Accueil, Comptes (liste et détail), Transactions, Import (assistant et revue), Rapprochement,
  Budget (mois, lignes, projets), Épargne, Projections, Réglages (catégories, règles, profils, sauvegardes).
- **Mobile d'abord pour la consultation et la saisie rapide** (soldes, budget du mois, saisie d'une
  dépense, catégorisation). L'**import et la revue d'import** sont conçus pour l'ordinateur, et
  restent simplement utilisables sur mobile.
- Montants : module `money.ts` (analyse d'une saisie « 1 234,56 » en centimes, formatage
  `Intl.NumberFormat('fr-FR')`) et pipe d'affichage, testés, sans arithmétique flottante.
- Stack et conventions : ADR-0019 (Angular, Angular Material, signals, un dossier par feature).
- Pas de PWA en V1 : elle suppose HTTPS (ADR-0020).

### Commandes (`justfile`)

| Commande | Effet |
|---|---|
| `just dev` | Sur le Mac : serveur Rust (rechargement automatique) + `ng serve`, base de démonstration (ADR-0018) |
| `just test` | `cargo test --workspace` + `ng test` |
| `just lint` | `cargo fmt --check`, `cargo clippy -- -D warnings`, `ng lint`, `prettier --check` |
| `just openapi` | Régénère `openapi.json` puis le client TypeScript |
| `just db-prepare` | `cargo sqlx prepare` (requêtes hors ligne pour la CI) |
| `just e2e` | Tests Playwright sur une instance de test |
| `just deploy` | Sur le serveur : récupère la version, sauvegarde la base, reconstruit et relance (ADR-0018) |

### Intégration continue

GitHub Actions (ou équivalent) : `lint`, `test`, vérification qu'`openapi.json`, le client généré et
`.sqlx/` sont à jour, build de l'image Docker. Tous ces contrôles sont bloquants.
