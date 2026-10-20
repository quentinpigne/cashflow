# ADR-0009 — Choix de la stack technique

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : architecture, roadmap, CLAUDE.md ; complété par ADR-0015 (déploiement), ADR-0016 (contrat d'API), ADR-0017 (base de données) et ADR-0018 (environnements)

## Contexte

Ce que le domaine exige, quelle que soit la stack :
- arithmétique **décimale exacte** et entiers 64 bits (ADR-0001) ;
- base embarquée à fichier unique (SQLite), avec migrations versionnées ;
- lecture CSV, XLSX et OFX ; client HTTP pour l'OCR optionnel ;
- un cœur métier pur et testable (ADR-0013), avec tests de propriété ;
- des écrans riches : revue d'import éditable, saisie de ventilation, graphiques.

Éléments décisifs exprimés le 2026-10-20 :
- le développement sera **principalement fait par IA** ; le propriétaire du projet apprécie Rust ;
- utilisation depuis **l'ordinateur et le téléphone** ;
- hébergement sur un **petit serveur personnel**, accessible **uniquement sur le réseau local** ;
- notifications **dans l'application** seulement.
- le code est relu par le propriétaire du projet, qui lit l'**Angular** avec le plus d'aisance. Ses
  priorités pour l'interface : un code homogène et facile à relire, une interface soignée sur mobile,
  peu de dépendances, des composants riches prêts à l'emploi.

## Options envisagées

1. **Rust + Tauri (application de bureau)** — ❌ Pas d'accès depuis le téléphone.
2. **Rust (axum) + SPA web, serveur auto-hébergé** — ✅ Accès multi-appareils. ✅ Le compilateur Rust
   est un garde-fou très efficace pour du code généré par IA : types, exhaustivité des `match`,
   absence de `null`, emprunts vérifiés. ✅ Binaire unique, faible empreinte mémoire, adapté à un
   petit serveur.
3. **Python repensé** — ✅ Vitesse. ❌ Garanties plus faibles, alors que le code sera majoritairement généré.
4. **TypeScript full-stack** — ❌ Pas de décimal natif, typage moins strict côté domaine.

Pour l'interface, au sein de l'option 2 :
- a. **React + TypeScript** — ✅ Écosystème le plus vaste. ❌ Une dizaine de bibliothèques à assembler et
  à suivre (routeur, données serveur, tableaux, formulaires, composants) ; style de code plus variable ;
  framework que le propriétaire relit moins facilement.
- b. **Angular + PrimeNG** — ✅ Composants très riches (table avec filtres et édition en ligne,
  graphiques). ❌ Éditeur tiers : montées de version majeures souvent cassantes, seule la dernière version
  maintenue gratuitement.
- c. **Angular + Angular Material** — ✅ Routeur, formulaires, HTTP, injection de dépendances et gestion
  d'état intégrés ; une seule façon de faire ; bibliothèque de composants officielle, publiée au rythme
  d'Angular ; Material 3 pensé pour le mobile. ❌ Table plus simple que celle de PrimeNG : l'écran de
  revue d'import demandera un peu plus de code.
- d. **SvelteKit** ou **Leptos (Rust → WebAssembly)** — ❌ Écosystème plus restreint, moins bien maîtrisé
  par l'IA, et inconnus du propriétaire.

## Décision

**Option 2c : serveur Rust + SPA Angular avec Angular Material**, servie par le même binaire.

### Backend (Rust, édition 2024, toolchain stable)

| Besoin | Choix |
|---|---|
| Serveur HTTP | `axum` + `tokio` + `tower-http` (compression, traces, fichiers statiques) |
| Base de données | SQLite (mode WAL, tables `STRICT`) via `sqlx`, sans ORM, requêtes vérifiées à la compilation, migrations `sqlx migrate` (ADR-0017) |
| Montants | Type maison `Money(i64)` ; calculs intermédiaires avec `rust_decimal` |
| Dates | `chrono` (`NaiveDate` pour les dates métier, `DateTime<Utc>` pour l'audit) |
| Identifiants | `uuid` v7, avec un *newtype* par entité |
| Sérialisation | `serde`, `serde_json` |
| Contrat d'API | `utoipa` (OpenAPI généré depuis le code), cf. ADR-0016 |
| Erreurs | `thiserror` dans les bibliothèques, `anyhow` uniquement dans le binaire |
| Journalisation | `tracing` + `tracing-subscriber` |
| Lecture de fichiers | `csv`, `encoding_rs` (windows-1252…), `calamine` (XLSX) ; OFX via un lecteur dédié, à base de `quick-xml` plus un prétraitement SGML pour l'OFX 1.x (crate existante à évaluer) |
| OCR (optionnel) | `reqwest` vers l'API du fournisseur (ADR-0007) |
| Authentification | `argon2` (hachage du mot de passe), session par cookie (ADR-0015) |
| Tests | `cargo test`, `proptest` (invariants), `insta` (golden files des lecteurs), SQLite temporaire pour l'intégration |
| Qualité | `rustfmt`, `clippy -D warnings` ; `clippy.toml` interdit `f32`/`f64` dans le crate `domain` (`disallowed-types`) ; `#![forbid(unsafe_code)]` |

### Frontend (Angular, TypeScript strict)

| Besoin | Choix |
|---|---|
| Framework | **Angular**, dernière version stable : composants *standalone*, *signals*, sans zone.js, syntaxe de contrôle `@if` / `@for` |
| Composants | **Angular Material** + CDK (Material 3) |
| Gestion d'état | **Signals** et services ; `httpResource` pour les données serveur. Pas de NgRx ; NgRx SignalStore pourra être introduit si un écran le justifie |
| Formulaires | Reactive Forms typés |
| Client d'API | Généré depuis `openapi.json` par `ng-openapi-gen` (dépendance de développement), jamais écrit à la main (ADR-0016) |
| Graphiques | ECharts via `ngx-echarts` |
| Installation mobile | PWA via `@angular/pwa`, quand l'application sera servie en HTTPS (ADR-0015) ; pas en V1 |
| Tests | Vitest (lanceur de tests d'Angular) ; Playwright pour les parcours de bout en bout (ADR-0018) |
| Qualité | TypeScript `strict` et `strictTemplates`, `angular-eslint`, Prettier |
| Gestionnaire de paquets | npm |

Conventions de l'interface :
- Un dossier par feature (`accounts`, `transactions`, `import`…), à l'image des modules du domaine.
- Composants en `ChangeDetectionStrategy.OnPush`, état exposé en signals.
- Montants : tout passe par `money.ts` (analyse de saisie, formatage `fr-FR`) et un pipe dédié ; aucune
  arithmétique sur les montants ailleurs.

Les versions exactes sont figées au moment de l'initialisation du dépôt (dernières versions stables),
puis mises à jour volontairement.

## Conséquences

- Deux langages : le contrat d'API généré (ADR-0016) est la frontière unique entre les deux.
  Aucun type n'est dupliqué à la main.
- Côté interface, les montants circulent en centimes entiers (`number`, sûr jusqu'à 2^53) : la
  conversion entre saisie et affichage est un module TypeScript testé, sans flottant pour l'arithmétique.
- Les dépendances d'exécution du front se limitent à Angular, Angular Material/CDK et ECharts ; leurs
  montées de version suivent celles d'Angular (`ng update`), deux fois par an.
- Les temps de compilation Rust sont un coût accepté. Le découpage en deux crates (ADR-0013) limite
  les recompilations.
- `sqlx` en mode hors ligne (`cargo sqlx prepare`, fichiers `.sqlx/` versionnés) : la CI compile sans base.
