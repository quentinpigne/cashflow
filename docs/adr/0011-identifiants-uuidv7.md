# ADR-0011 — Identifiants UUIDv7

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : modèle de données, export (ADR-0008)

## Contexte

Les entités sont créées localement, exportées et réimportées (ADR-0008), et liées entre elles
depuis des documents JSON (règles de catégorisation, scénarios). Des identifiants auto-incrémentés
entrent en collision lors d'une fusion ou d'un réimport, et révèlent l'ordre et le volume des données.

## Options envisagées

1. **Entiers auto-incrémentés** — ✅ compacts. ❌ Collisions à l'import et à la fusion, ils dépendent de la base.
2. **UUIDv4** — ✅ uniques. ❌ Aléatoires : mauvaise localité dans les index, pas de tri temporel.
3. **UUIDv7** — ✅ uniques, générés côté application, triables par date de création, bonne localité d'index.

## Décision

Tous les identifiants sont des **UUIDv7 générés par le cœur métier**, jamais par la base. Dans le
code, ils sont typés par entité (`AccountId`, `TransactionId`…) pour qu'on ne puisse pas les confondre.

## Conséquences

- Un export JSON se réimporte sans renumérotation.
- Stockage : 16 octets en binaire, ou 36 caractères en texte si la lisibilité en base est préférée.
  Retenu : `TEXT` (ADR-0017, modèle de données).
