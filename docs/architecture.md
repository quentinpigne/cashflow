# Architecture cible

Statut : **Validée** (2026-10-20) — principes indépendants de la stack, projection sur Rust + Angular en fin de document.

## Vue d'ensemble

Monolithe en couches, autour d'un **cœur de calcul pur** (ADR-0013). Une seule base SQLite à fichier
unique. Une seule API (REST + OpenAPI, ADR-0016) consommée par une SPA Angular servie par le même binaire.

```
┌─────────────────────────────────────────────────────────────────────────┐
│ api : routes REST (axum, DTO, OpenAPI) · sous-commandes CLI (export,     │
│       sauvegarde, migrations)                                            │
└───────────────┬─────────────────────────────────────────────────────────┘
                │ appelle
┌───────────────▼─────────────────────────────────────────────────────────┐
│ services : un par cas d'usage                                            │
│  CreateTransaction · SplitTransaction · LinkTransfer · StartImport       │
│  ReviewImportLine · CommitImport · RevertImport · AddCheckpoint          │
│  Reconcile · Unlock · ComputeInterest · RunProjection · ExportAll …      │
│  → une transaction SQLite par service : charger, calculer, écrire        │
└───────┬───────────────────────────────────────────────┬─────────────────┘
        │ calcule avec                                   │ lit et écrit avec
┌───────▼──────────────────────────────┐   ┌─────────────▼───────────────┐
│ domain (pur, sans I/O)                │   │ db : requêtes sqlx (SQLite) │
│  Types : Money, Rate, Ids, Date       │   ├─────────────────────────────┤
│  Entités + invariants                 │   │ readers : lecteurs de       │
│  Moteurs :                            │   │  relevés CSV/XLSX/OFX/PDF   │
│   · BalanceEngine (soldes à date)     │   │  (trait StatementReader),   │
│   · SplitAllocator (% → montants)     │   │  OCR optionnel (ADR-0007)   │
│   · Normalizer (montants, dates)      │   ├─────────────────────────────┤
│   · Fingerprinter                     │   │ files : fichiers source,    │
│   · Matcher (correspondances)         │   │  sauvegardes                │
│   · RuleEngine (catégorisation)       │   └─────────────────────────────┘
│   · ReconciliationEngine (écarts)     │
│   · InterestEngine (quinzaines…)      │
│   · PerformanceEngine (TRI, TWR)      │
│   · ProjectionEngine                  │
│   · RecurrenceExpander                │
│   · BudgetEngine · AlertEvaluator     │
│   · Indicators (tableau de bord)      │
└───────────────────────────────────────┘
```

Le domaine ne connaît ni la base, ni les fichiers, ni l'horloge : la date du jour lui est passée en
paramètre. Les traits sont réservés aux endroits où plusieurs implémentations existent vraiment
(lecteurs de relevés, OCR).

## Modules du domaine (contextes)

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
dépend pas de `reconciliation` : la création des points de contrôle à la validation passe par le
service `CommitImport`.

## Flux clés

### Import (spec 03)

```
StartImport(fichier, compte, profil)
  → StatementReader.read(fichier, profil)     [readers : CSV/XLSX/OFX/PDF]
  → Normalizer → Fingerprinter                 [domain]
  → Matcher(lignes, transactions candidates)   [domain ; candidats chargés par le service]
  → RuleEngine(lignes, règles)                 [domain]
  → persistance ImportBatch + ImportLine (REVIEWING)
ReviewImportLine(...)                          → mise à jour d'une décision
CommitImport(batch)                            → 1 transaction BDD : transactions, pointages,
                                                 virements, points de contrôle, statut COMMITTED
```

### Solde et rapprochement (spec 04)

`BalanceEngine` reçoit le solde d'ouverture et les montants datés, et produit le solde à une date ou
une série. Les agrégats SQL (Σ par compte jusqu'à D) sont une optimisation du module `db` : ils
doivent produire exactement le même résultat que le moteur, ce que vérifie un test de cohérence.

## Gestion des erreurs

- Les violations de règles métier sont des **erreurs typées** du domaine (ex. `SplitSumMismatch`,
  `TransactionLocked{checkpoint_id}`, `AccountClosed`), jamais des chaînes ni des exceptions génériques.
  L'interface les traduit en messages ; chacune cite la règle `R-*` concernée.
- Les erreurs de lecture de fichier sont des **données** (champ `errors` de `ImportLine`), pas des
  exceptions (R-IMP-3).

## Stratégie de test

| Niveau | Cible | Outils |
|---|---|---|
| Unitaire | Moteurs du domaine, règle par règle (`R-*`) | `cargo test` ; Vitest (`ng test`) pour `money.ts` et la logique d'interface |
| Propriétés | Invariants : Σ ventilations, solde = ouverture + Σ, réimport idempotent, patrimoine invariant par virement, annulation d'import = état initial | `proptest` |
| Golden files | Lecteurs de fichiers : `fixtures/<banque>/<cas>.csv` → `expected.json` | `insta`, fichiers **anonymisés** uniquement |
| Intégration | Services avec une SQLite réelle (en mémoire), sans mocks | — |
| Aller-retour | Export JSON → import → export identique | — |
| Bout en bout | Parcours critiques : importer, revoir, valider, rapprocher | Playwright |

Les cas de référence chiffrés des specs (intérêts, TRI, ventilations, indicateurs) sont repris
**tels quels** comme tests.

## Sécurité et confidentialité

- Aucune donnée réelle ni aucun secret dans le dépôt (cf. CLAUDE.md). `.gitignore` initial : base de
  données, fichiers importés, `.env*` sauf `.env.example`.
- Accès : réseau local uniquement, mot de passe unique et session par cookie (ADR-0015). HTTP simple en
  V1, HTTPS servi par axum plus tard (ADR-0015).
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

Stack : ADR-0009 (Rust + Angular). Architecture : ADR-0013. Déploiement : ADR-0015, ADR-0018. API : ADR-0016.

### Organisation du dépôt

```
cashflow/
├── Cargo.toml                 ← workspace Rust
├── crates/
│   ├── domain/                ← cœur de calcul pur (ADR-0013)
│   │   └── src/{shared,accounts,ledger,import,reconciliation,
│   │            budgeting,savings,projection,reporting}.rs + dossiers homonymes
│   └── server/                ← binaire `cashflow` et ses sous-commandes
│       ├── src/routes.rs + routes/ ← assemblage des routes axum par domaine, contrat OpenAPI, fichiers statiques
│       ├── src/handlers.rs + handlers/ ← handlers axum, DTO, description OpenAPI (utoipa)
│       ├── src/middleware.rs       ← session, alertes de budget
│       ├── src/services.rs + services/  ← un service par cas d'usage
│       ├── src/db.rs + db/         ← requêtes sqlx
│       ├── src/readers.rs + readers/   ← lecteurs CSV/XLSX/OFX/PDF, OCR
│       ├── migrations/        ← migrations sqlx (immuables une fois publiées)
│       └── seed/              ← données initiales (JSON), sans donnée personnelle
├── web/                       ← SPA Angular (Angular CLI)
│   └── src/app/api/           ← client généré depuis openapi.json (jamais édité à la main)
├── openapi.json               ← contrat d'API versionné (ADR-0016)
├── .sqlx/                     ← cache des requêtes sqlx, pour compiler sans base (ADR-0017)
├── fixtures/                  ← fichiers d'import anonymisés + résultats attendus (golden), données de démonstration
├── deploy/                    ← Dockerfile, compose.yaml, guide d'installation
├── docs/                      ← conception : docs transverses, specs/, adr/
├── README.md · CLAUDE.md
```

### Conventions Rust

- Édition **2024** pour tous les crates (`edition = "2024"` dans `[workspace.package]`).
- Modules en **fichiers homonymes**, sans `mod.rs` :
  ```
  crates/domain/src/
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
  Les flottants sont interdits dans `domain` par `disallowed-types`, dans `crates/domain/clippy.toml`. Ce
  fichier remplace celui de la racine pour ce crate (clippy ne fusionne pas les deux) : il doit donc
  reprendre aussi les options `allow-*-in-tests`.

### Dépendances entre crates

```
server ──► domain
```

- `domain` ne dépend d'aucun crate d'I/O (ni `tokio`, ni `sqlx`, ni `axum`, ni `reqwest`). Ses dépendances
  autorisées : `rust_decimal` (avec sa feature `maths` : logarithme et exponentielle décimaux du TRI,
  R-EPA-22), `chrono`, `uuid`, `serde`, `thiserror`, `sha2` et `hex` (empreintes des
  lignes d'import, R-IMP-5), et `utoipa` derrière la feature
  optionnelle `openapi` (dérivation des schémas des types exposés par l'API, sans I/O). Les flottants y
  sont interdits par clippy (`disallowed-types`). La frontière de crate fait respecter la pureté par le
  compilateur.
- Dans `server`, les dépendances entre modules vont dans un seul sens : `routes` → `handlers` → `services` →
  `db` / `readers`, sur le découpage d'axum. Les routes ne font qu'assembler ; les handlers ne calculent pas et
  n'exécutent pas de requête ; les services ne contiennent pas de calcul métier.
- Les relevés PDF (IMP-10) sont lus avec `pdf-extract` (pur Rust, au-dessus de `lopdf`), qui donne le texte et
  la position de chaque élément sans bibliothèque native ni service tiers (ADR-0007). `pdfium-render` a été
  écarté : il embarque la bibliothèque PDFium dans l'image. Les paniques éventuelles de la bibliothèque sont
  confinées à la lecture du fichier, qui est alors refusé.
- Chaque service s'exécute dans **une transaction SQLite**, ouverte et validée par le service.

### Correspondance des modules

| Module du domaine | Moteurs (`domain`) | Accès aux données et fichiers (`server`) | Routes (`server`, `/api/v1`) |
|---|---|---|---|
| `accounts` | — | `db::accounts` | `/institutions`, `/accounts`, `/accounts/{id}/valuations` |
| `ledger` | `BalanceEngine`, `SplitAllocator`, `RecurrenceExpander` | `db::ledger` | `/transactions`, `/categories`, `/transfers`, `/recurring-rules` |
| `import` | `Normalizer`, `Fingerprinter`, `Matcher`, `RuleEngine` | `db::import` ; `readers::{csv, xlsx, ofx, lcl_pdf, ocr}` | `/import-profiles`, `/imports`, `/imports/{id}/lines`, `/rules` |
| `reconciliation` | `ReconciliationEngine` | `db::checkpoints` | `/accounts/{id}/checkpoints` |
| `budgeting` | `BudgetEngine`, `AlertEvaluator` | `db::budget`, `db::notifications` | `/budget/lines`, `/budget/months/{yyyy-mm}`, `/projects`, `/notifications` |
| `savings` | `InterestEngine`, `PerformanceEngine` | `db::rates` | `/rate-schedules`, `/accounts/{id}/interest`, `/accounts/{id}/performance` |
| `projection` | `ProjectionEngine` | `db::scenarios` | `/scenarios`, `/projections` |
| `reporting` | `Indicators` | — | `/dashboard`, `/reports/*` |
| — | — | export JSON | `/export` (et sous-commande `cashflow export`) |

### SQLite

Décision et précautions : ADR-0017.

- SQLite embarqué (≥ 3.37), tables `STRICT`, `journal_mode = WAL`, `foreign_keys = ON`,
  `synchronous = NORMAL`, `busy_timeout = 5000` ; une seule connexion en écriture.
- Types physiques : cf. `modele-de-donnees.md` § « Correspondance physique SQLite ».
- L'invariant Σ ventilations est garanti par le domaine **et** contrôlé avant la validation de chaque
  transaction SQLite qui touche des transactions, en production comme en test.
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
- Stack et conventions : ADR-0009 (Angular, Angular Material, signals, un dossier par feature).
- Navigation et écrans : [ecrans.md](ecrans.md).
- Pas de PWA en V1 : elle suppose HTTPS (ADR-0015).

### Commandes

Pas d'outil de commandes (ADR-0018) : les commandes `cargo` se lancent à la racine, les commandes `npm` /
`npx` dans `web/`.

| Tâche | Commandes |
|---|---|
| Base de développement (Mac) | `cargo run -p cashflow-server -- demo` : base `data/` remplie du jeu de démonstration (mot de passe `demo-cashflow`) |
| Lancer en développement | `cargo run -p cashflow-server -- serve` (API sur le port 3000) et `npm start` (`ng serve`, qui relaie `/api` vers le serveur) |
| Tests | `cargo test --workspace` ; `npx ng test --watch=false` |
| Lint et format | `cargo fmt --check` ; `cargo clippy --workspace --all-targets -- -D warnings` ; `npx ng lint` ; `npm run format:check` |
| Contrat d'API | `cargo run -p cashflow-server -- openapi > openapi.json`, puis `npm run openapi` (client généré) |
| Requêtes sqlx | `cargo sqlx prepare --workspace` (cache `.sqlx/`, cf. `.env.example` pour la base de schéma) |
| Bout en bout | `npx ng build` puis `npm run e2e` (Playwright lance le serveur sur un jeu de démonstration à date figée) |
| Déploiement (serveur) | `deploy/deploy.sh` — procédure et retour arrière dans `deploy/README.md` (ADR-0018) |

### Intégration continue

GitHub Actions (`.github/workflows/ci.yml`) : lint et tests Rust et TypeScript, vérification
qu'`openapi.json`, le client généré et `.sqlx/` sont à jour, tests de bout en bout Playwright, build de
l'image Docker. Tous ces contrôles sont bloquants ; la CI ne déploie pas (ADR-0018).
