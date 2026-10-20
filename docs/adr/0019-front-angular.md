# ADR-0019 — Interface en Angular avec Angular Material

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : remplace la partie « Frontend » de l'ADR-0009 ; ADR-0016, ADR-0018, architecture.md

## Contexte

L'ADR-0009 retenait une SPA React assemblée à partir d'une dizaine de bibliothèques (TanStack
Router/Query/Table, react-hook-form, zod, shadcn/ui, Recharts…).

Le code est écrit par une IA et relu par le propriétaire du projet, qui lit l'Angular avec le plus
d'aisance. Ses priorités pour l'interface : un code homogène et facile à relire, une interface soignée
sur mobile, peu de dépendances et des composants riches prêts à l'emploi.

## Options envisagées

1. **React** (ADR-0009). ✅ Écosystème le plus vaste. ❌ Chaque besoin est une bibliothèque à choisir et
   à suivre ; style de code plus variable ; framework que le propriétaire relit moins facilement.
2. **Angular + PrimeNG**. ✅ Composants très riches (table avec filtres et édition en ligne, graphiques).
   ❌ Éditeur tiers : montées de version majeures souvent cassantes, seule la dernière version maintenue
   gratuitement.
3. **Angular + Angular Material**. ✅ Routeur, formulaires, HTTP, injection de dépendances et gestion
   d'état intégrés ; bibliothèque de composants officielle, publiée au rythme d'Angular ; Material 3
   pensé pour le mobile. ❌ Table plus simple que celle de PrimeNG : l'écran de revue d'import demandera
   un peu plus de code.

## Décision

Option 3. La SPA reste servie par le binaire Rust (ADR-0015).

| Besoin | Choix |
|---|---|
| Framework | **Angular**, dernière version stable : composants *standalone*, *signals*, sans zone.js, syntaxe de contrôle `@if` / `@for` |
| Composants | **Angular Material** + CDK (Material 3) |
| Gestion d'état | **Signals** et services ; `httpResource` pour les données serveur. Pas de NgRx ; NgRx SignalStore pourra être introduit par un ADR si un écran le justifie |
| Formulaires | Reactive Forms typés |
| Client d'API | Généré depuis `openapi.json` par `ng-openapi-gen` (dépendance de développement), jamais écrit à la main (ADR-0016) |
| Graphiques | ECharts via `ngx-echarts` |
| Tests | Vitest (lanceur de tests d'Angular) ; Playwright pour les parcours de bout en bout (ADR-0018) |
| Qualité | TypeScript `strict` et `strictTemplates`, `angular-eslint`, Prettier |
| Gestionnaire de paquets | npm |

Conventions :
- Un dossier par feature (`accounts`, `transactions`, `import`…), à l'image des modules du cœur.
- Composants en `ChangeDetectionStrategy.OnPush`, état exposé en signals.
- Montants : tout passe par `money.ts` (analyse de saisie, formatage `fr-FR`) et un pipe dédié ;
  aucune arithmétique sur les montants ailleurs.

## Conséquences

- Les dépendances d'exécution se limitent à Angular, Angular Material/CDK et ECharts.
- Les montées de version suivent celles d'Angular (`ng update`), deux fois par an.
- L'installation sur le téléphone en PWA (`@angular/pwa`) est prévue, mais suppose HTTPS : elle attend
  la bascule décrite dans l'ADR-0020.
- `architecture.md` (organisation du dépôt, interface, commandes, tests) et `CLAUDE.md` sont mis à jour.
