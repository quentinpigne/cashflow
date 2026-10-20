# ADR-0016 — Une API REST unique, décrite en OpenAPI, avec un client généré

- **Statut** : Accepté
- **Date** : 2026-10-20
- **Concerne** : architecture, ADR-0009

## Contexte

Le projet a deux langages (Rust, TypeScript) et sera développé principalement par IA : tout type
dupliqué à la main entre les deux finira par diverger.

## Options envisagées

1. **GraphQL** — ✅ Requêtes flexibles. ❌ Complexité (dataloaders, cache) injustifiée pour un seul client.
2. **REST + OpenAPI généré depuis le code Rust, client TypeScript généré** — ✅ Une source de vérité
   (le code serveur), types partagés automatiquement, documentation interactive gratuite.
3. **RPC typé partagé (types générés par `ts-rs` ou `specta`, sans OpenAPI)** — ✅ Léger. ❌ Pas de
   description standard des routes ni des erreurs.

## Décision

Option 2 :
- L'API est en **REST/JSON** sous `/api/v1`, décrite par `utoipa` à partir des handlers et des DTO Rust.
  Le document `openapi.json` est **versionné** dans le dépôt et régénéré par une commande dédiée.
- Le client TypeScript est **généré** depuis `openapi.json` (`openapi-typescript`). Le code de
  l'interface n'écrit jamais un type d'API à la main.
- La **CI échoue** si `openapi.json` ou le client généré ne sont pas à jour.
- Conventions :
  - montants en **centimes entiers** (`amount_minor`), taux en **millionièmes** (`rate_ppm`) ;
  - dates en `YYYY-MM-DD`, identifiants en chaînes UUID ;
  - erreurs au format `application/problem+json` (RFC 9457), avec un champ `code` stable
    (ex. `SPLIT_SUM_MISMATCH`, `TRANSACTION_LOCKED`) et, si pertinent, la règle `R-*` concernée ;
  - pagination par curseur pour les listes de transactions.
- Les DTO de l'API sont **distincts** des entités du cœur : le crate `server` fait la conversion.

## Conséquences

- Une modification d'API est visible dans le diff d'`openapi.json`, et l'interface casse à la
  compilation TypeScript : pas de dérive silencieuse.
- La documentation interactive (Swagger UI ou Scalar) est servie en développement uniquement.
