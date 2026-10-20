# ADR-0009 — Choix de la stack technique

- **Statut** : Accepté ; partie « Frontend » remplacée par ADR-0019
- **Date** : 2026-10-20
- **Concerne** : architecture, roadmap, CLAUDE.md ; complété par ADR-0015 (déploiement), ADR-0016 (contrat d'API) et ADR-0017 (base de données)

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

## Options envisagées

1. **Rust + Tauri (application de bureau)** — ❌ Pas d'accès depuis le téléphone.
2. **Rust (axum) + SPA web, serveur auto-hébergé** — ✅ Accès multi-appareils. ✅ Le compilateur Rust
   est un garde-fou très efficace pour du code généré par IA : types, exhaustivité des `match`,
   absence de `null`, emprunts vérifiés. ✅ Binaire unique, faible empreinte mémoire, adapté à un
   petit serveur.
3. **Python repensé** — ✅ Vitesse. ❌ Garanties plus faibles, alors que le code sera majoritairement généré.
4. **TypeScript full-stack** — ❌ Pas de décimal natif, typage moins strict côté domaine.

Pour l'interface, au sein de l'option 2 :
- a. **React + TypeScript** — ✅ Écosystème le plus riche (tableaux éditables, graphiques, formulaires)
  et le mieux maîtrisé par les modèles d'IA. ❌ Deux langages.
- b. **SvelteKit** — ✅ Concis. ❌ Écosystème et maîtrise par l'IA un cran en dessous.
- c. **Leptos (Rust → WebAssembly)** — ✅ Un seul langage. ❌ Écosystème jeune, API mouvante, moins
  bien maîtrisé par l'IA.

## Décision

**Option 2a : serveur Rust + SPA React/TypeScript**, servie par le même binaire.

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
| Qualité | `rustfmt`, `clippy -D warnings` ; `clippy.toml` interdit `f32`/`f64` dans le crate `core` (`disallowed-types`) ; `#![forbid(unsafe_code)]` |

### Frontend (TypeScript strict)

| Besoin | Choix |
|---|---|
| Build | Vite |
| UI | React, Tailwind CSS, composants shadcn/ui (Radix) |
| Routage | TanStack Router |
| Données serveur | TanStack Query + client généré depuis l'OpenAPI (`openapi-typescript` + `openapi-fetch`) |
| Tableaux | TanStack Table (revue d'import, transactions) |
| Formulaires | react-hook-form + zod |
| Graphiques | Recharts |
| Installation mobile | PWA (manifeste + service worker minimal) via `vite-plugin-pwa`, ce qui suppose HTTPS (ADR-0015) |
| Tests | Vitest + Testing Library ; Playwright pour les parcours de bout en bout |
| Qualité | `tsc --strict`, Biome (lint et format) |

Les versions exactes sont figées au moment de l'initialisation du dépôt (dernières versions stables),
puis mises à jour volontairement.

## Conséquences

- Deux langages : le contrat d'API généré (ADR-0016) est la frontière unique entre les deux.
  Aucun type n'est dupliqué à la main.
- Côté interface, les montants circulent en centimes entiers (`number`, sûr jusqu'à 2^53) : la
  conversion entre saisie et affichage est un module TypeScript testé, sans flottant pour l'arithmétique.
- Les temps de compilation Rust sont un coût accepté. Le découpage en crates (cf. architecture) limite
  les recompilations.
- `sqlx` en mode hors ligne (`cargo sqlx prepare`, fichiers `.sqlx/` versionnés) : la CI compile sans base.
